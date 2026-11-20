//! The operations engine (DESIGN.md §7.1, Phase 2): every change to files
//! goes through here, so every change is journaled and can be undone.
//!
//! Requests run one at a time on a worker thread. Events (progress, a name
//! clash that needs a decision, the finished report) are delivered on the
//! default GLib main context through the handler given to [`Engine::start`].
//!
//! AGENTS.md rule 10: no other module may move, copy, rename, trash or
//! delete files. Scenario E2-13 checks it.

mod fsx;
pub mod journal;
pub mod style;
pub mod trash;
mod worker;

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

pub use fsx::free_name;
pub use trash::{Trash, TrashItem};
pub use worker::{Context, Worker, touched_folders};

use gio::prelude::*;

use crate::store::Store;

/// What the user asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Copy {
        sources: Vec<PathBuf>,
        dest: PathBuf,
    },
    Move {
        sources: Vec<PathBuf>,
        dest: PathBuf,
    },
    Duplicate {
        sources: Vec<PathBuf>,
    },
    Rename {
        source: PathBuf,
        name: String,
    },
    NewFolder {
        parent: PathBuf,
        name: String,
    },
    Trash {
        sources: Vec<PathBuf>,
    },
    /// Items inside a Trash `files/` folder, put back where they were.
    Restore {
        items: Vec<PathBuf>,
    },
    /// Deletes everything in the home Trash. Cannot be undone.
    EmptyTrash,
    /// A plan's moves (Tidy and the like), as one batch: kind folders that
    /// are missing are made (and removed again by Undo); a name already
    /// taken gets a free one ("Keep both").
    Apply {
        kind: String,
        moves: Vec<(PathBuf, PathBuf)>,
    },
    Undo,
    Redo,
}

impl Request {
    /// The batch kind stored in the journal.
    pub fn kind(&self) -> &'static str {
        match self {
            Request::Copy { .. } => "copy",
            Request::Move { .. } => "move",
            Request::Duplicate { .. } => "duplicate",
            Request::Rename { .. } => "rename",
            Request::NewFolder { .. } => "new-folder",
            Request::Trash { .. } => "trash",
            Request::Restore { .. } => "restore",
            Request::EmptyTrash => "empty-trash",
            Request::Apply { .. } => "apply",
            Request::Undo => "undo",
            Request::Redo => "redo",
        }
    }

    fn undoable(&self) -> bool {
        !matches!(self, Request::EmptyTrash | Request::Undo | Request::Redo)
    }

    /// What progress is measured over.
    pub fn sources(&self) -> Vec<PathBuf> {
        match self {
            Request::Copy { sources, .. }
            | Request::Move { sources, .. }
            | Request::Duplicate { sources }
            | Request::Trash { sources } => sources.clone(),
            Request::Apply { moves, .. } => moves.iter().map(|(from, _)| from.clone()).collect(),
            _ => Vec::new(),
        }
    }
}

/// What a drop does (P2-T05), as Files does it: a move within one
/// filesystem, a copy onto another; holding Ctrl always copies. Dropping
/// items onto the folder they are already in does nothing.
pub fn drop_request(sources: &[PathBuf], dest: &Path, ctrl: bool) -> Option<Request> {
    let sources: Vec<PathBuf> = sources
        .iter()
        .filter(|s| s.parent() != Some(dest) && !dest.starts_with(s))
        .cloned()
        .collect();
    if sources.is_empty() {
        return None;
    }
    let same_device = sources.iter().all(|s| fsx::same_device(s, dest));
    if ctrl || !same_device {
        Some(Request::Copy {
            sources,
            dest: dest.to_path_buf(),
        })
    } else {
        Some(Request::Move {
            sources,
            dest: dest.to_path_buf(),
        })
    }
}

/// The answer to a name clash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// Give the new item a free name: "name (2).ext".
    KeepBoth,
    /// Send the existing item to the Trash (undoable), then put the new one.
    Replace,
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Answer {
    pub choice: Choice,
    pub apply_to_all: bool,
}

/// One item that could not be done (or undone), and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub path: PathBuf,
    pub reason: FailReason,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailReason {
    NameTaken,
    Gone,
    PermissionDenied,
    NoSpace,
    ReadOnly,
    Other,
}

impl Failure {
    fn new(path: &Path, error: &io::Error) -> Self {
        let reason = match error.kind() {
            io::ErrorKind::AlreadyExists => FailReason::NameTaken,
            io::ErrorKind::NotFound => FailReason::Gone,
            io::ErrorKind::PermissionDenied => FailReason::PermissionDenied,
            io::ErrorKind::StorageFull => FailReason::NoSpace,
            io::ErrorKind::ReadOnlyFilesystem => FailReason::ReadOnly,
            _ => FailReason::Other,
        };
        Self {
            path: path.to_path_buf(),
            reason,
            message: error.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Forward,
    Undo,
    Redo,
}

/// What a finished request did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub kind: String,
    pub outcome: Outcome,
    /// The journal batch (None when nothing was recorded).
    pub batch: Option<i64>,
    /// The batch's summary (for Undo and Redo: the batch that was reverted).
    pub summary: String,
    pub done: usize,
    pub skipped: usize,
    pub failed: Vec<Failure>,
    pub cancelled: bool,
    /// Items now in place: new copies, moved or renamed items, restored items.
    pub created: Vec<PathBuf>,
}

impl Report {
    fn new(kind: &str, batch: Option<i64>) -> Self {
        Self {
            kind: kind.to_string(),
            outcome: Outcome::Forward,
            batch,
            summary: String::new(),
            done: 0,
            skipped: 0,
            failed: Vec::new(),
            cancelled: false,
            created: Vec::new(),
        }
    }

    fn error(kind: &str, error: io::Error) -> Self {
        let mut report = Self::new(kind, None);
        report.failed.push(Failure::new(Path::new(""), &error));
        report
    }

    fn finish(mut self, outcome: Outcome) -> Self {
        self.outcome = outcome;
        self
    }
}

pub type JobId = u64;

/// Something the UI should know about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Started {
        job: JobId,
    },
    Progress {
        job: JobId,
        bytes: u64,
        total_bytes: u64,
        items: u64,
        total_items: u64,
        current: String,
    },
    /// A name exists where an item is going; answer with [`Engine::answer`].
    Clash {
        job: JobId,
        name: String,
        folder: PathBuf,
    },
    Finished {
        job: JobId,
        report: Report,
    },
}

type Handler = Arc<dyn Fn(Event) + Send + Sync>;

struct Job {
    id: JobId,
    request: Request,
    summary: String,
    cancel: gio::Cancellable,
}

struct Shared {
    cancels: HashMap<JobId, gio::Cancellable>,
    answers: HashMap<JobId, Sender<Answer>>,
}

/// The queue: one worker thread, jobs in order.
pub struct Engine {
    jobs: Sender<Job>,
    shared: Arc<Mutex<Shared>>,
    next: JobId,
}

struct ThreadContext {
    job: JobId,
    cancel: gio::Cancellable,
    handler: Handler,
    shared: Arc<Mutex<Shared>>,
    last: std::time::Instant,
}

fn deliver(handler: &Handler, event: Event) {
    let handler = handler.clone();
    glib::MainContext::default().invoke(move || handler(event));
}

impl Context for ThreadContext {
    fn progress(
        &mut self,
        bytes: u64,
        total_bytes: u64,
        items: u64,
        total_items: u64,
        current: &str,
    ) {
        // At most ten updates a second.
        if self.last.elapsed() < std::time::Duration::from_millis(100) && bytes < total_bytes {
            return;
        }
        self.last = std::time::Instant::now();
        deliver(
            &self.handler,
            Event::Progress {
                job: self.job,
                bytes,
                total_bytes,
                items,
                total_items,
                current: current.to_string(),
            },
        );
    }

    fn ask(&mut self, name: &str, folder: &Path) -> Answer {
        let (tx, rx): (Sender<Answer>, Receiver<Answer>) = channel();
        if let Ok(mut shared) = self.shared.lock() {
            shared.answers.insert(self.job, tx);
        }
        deliver(
            &self.handler,
            Event::Clash {
                job: self.job,
                name: name.to_string(),
                folder: folder.to_path_buf(),
            },
        );
        // Cancelling the job while asking counts as Skip for every item.
        let skip = Answer {
            choice: Choice::Skip,
            apply_to_all: true,
        };
        loop {
            match rx.recv_timeout(std::time::Duration::from_millis(200)) {
                Ok(answer) => return answer,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) if !self.cancel.is_cancelled() => {
                    continue;
                }
                Err(_) => return skip,
            }
        }
    }

    fn cancellable(&self) -> &gio::Cancellable {
        &self.cancel
    }
}

impl Engine {
    /// Opens the journal at `db`, marks batches a crash left open as
    /// partial, and starts the worker. `step_delay` slows every step down
    /// (end-to-end tests use it to interrupt a batch midway). Returns the engine and the batches
    /// that were interrupted (to report).
    pub fn start(
        db: PathBuf,
        trash: Trash,
        step_delay: Option<std::time::Duration>,
        handler: impl Fn(Event) + Send + Sync + 'static,
    ) -> io::Result<(Self, Vec<journal::Batch>)> {
        let store = Store::open(&db).map_err(io::Error::other)?;
        let interrupted = store.recover().map_err(io::Error::other)?;
        let handler: Handler = Arc::new(handler);
        let shared = Arc::new(Mutex::new(Shared {
            cancels: HashMap::new(),
            answers: HashMap::new(),
        }));
        let (tx, rx) = channel::<Job>();
        let thread_handler = handler.clone();
        let thread_shared = shared.clone();
        std::thread::Builder::new()
            .name("cairn-ops".into())
            .spawn(move || {
                let mut worker = Worker::new(store, trash);
                worker.step_delay = step_delay;
                for job in rx {
                    deliver(&thread_handler, Event::Started { job: job.id });
                    let mut ctx = ThreadContext {
                        job: job.id,
                        cancel: job.cancel.clone(),
                        handler: thread_handler.clone(),
                        shared: thread_shared.clone(),
                        last: std::time::Instant::now() - std::time::Duration::from_secs(1),
                    };
                    let report = worker.run(&job.request, &job.summary, &mut ctx);
                    if let Ok(mut shared) = thread_shared.lock() {
                        shared.cancels.remove(&job.id);
                        shared.answers.remove(&job.id);
                    }
                    deliver(
                        &thread_handler,
                        Event::Finished {
                            job: job.id,
                            report,
                        },
                    );
                }
            })?;
        Ok((
            Self {
                jobs: tx,
                shared,
                next: 1,
            },
            interrupted,
        ))
    }

    /// Queues a request. `summary` is the user-visible description kept in
    /// the journal (the Edit menu shows it as "Undo <summary>").
    pub fn submit(&mut self, request: Request, summary: &str) -> JobId {
        let id = self.next;
        self.next += 1;
        let cancel = gio::Cancellable::new();
        if let Ok(mut shared) = self.shared.lock() {
            shared.cancels.insert(id, cancel.clone());
        }
        // No event here: submit runs on the main thread, where `invoke` would
        // call the handler at once, inside the caller's own borrow.
        let _ = self.jobs.send(Job {
            id,
            request,
            summary: summary.to_string(),
            cancel,
        });
        id
    }

    pub fn cancel(&self, job: JobId) {
        if let Ok(shared) = self.shared.lock()
            && let Some(cancel) = shared.cancels.get(&job)
        {
            cancel.cancel();
        }
    }

    pub fn answer(&self, job: JobId, answer: Answer) {
        if let Ok(mut shared) = self.shared.lock()
            && let Some(tx) = shared.answers.remove(&job)
        {
            let _ = tx.send(answer);
        }
    }
}
