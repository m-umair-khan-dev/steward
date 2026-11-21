//! File operations from the window (Phase 2): new folder, rename in place,
//! Trash, duplicate, clipboard, undo and redo, the queue, name clashes and
//! the Trash view. Every change goes through `cairn_core::ops`.

use std::path::{Path, PathBuf};

use gettextrs::gettext;
use gio::prelude::*;
use serde_json::{Value, json};

use cairn_core::clip;
use cairn_core::ops::{
    Answer, Choice, Engine, Event as OpEvent, FailReason, JobId, Outcome, Report, Request, Trash,
};
use cairn_core::store::Store;

use super::{Controller, Event, dirty, later};
use crate::{app_data, format, paths};

/// A job in the queue, as the header's progress list shows it.
#[derive(Debug, Clone)]
pub struct JobView {
    id: JobId,
    summary: String,
    running: bool,
    bytes: u64,
    total_bytes: u64,
    items: u64,
    total_items: u64,
    current: String,
}

#[derive(Default)]
pub struct Ops {
    engine: Option<Engine>,
    trash: Option<Trash>,
    jobs: Vec<JobView>,
    /// The job waiting for a clash answer, the name and the folder.
    clash: Option<(JobId, String, String)>,
    /// Our own copy of the last Copy or Cut, used when the system clipboard
    /// cannot be read (no focus) or holds nothing.
    clipboard: Option<clip::Payload>,
    /// The item being renamed in place (URI).
    pub renaming: Option<String>,
    /// Items to select once they appear (URIs).
    pending_select: Vec<String>,
    /// What New Folder made, to rename it once it shows.
    rename_when_shown: Option<String>,
    interrupted: Vec<String>,
    last_report: Value,
}

fn uri_of(path: &Path) -> String {
    gio::File::for_path(path).uri().to_string()
}

fn path_of(uri: &str) -> Option<PathBuf> {
    gio::File::for_uri(uri).path()
}

/// "“a.txt”" or "3 items".
fn things(paths: &[PathBuf]) -> String {
    match paths {
        [one] => format!("“{}”", display_name(one)),
        many => format::items(many.len() as u32),
    }
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn reason(reason: FailReason) -> String {
    match reason {
        FailReason::NameTaken => gettext("the name is taken"),
        FailReason::Gone => gettext("it is no longer there"),
        FailReason::PermissionDenied => gettext("you do not have permission"),
        FailReason::NoSpace => gettext("the disk is full"),
        FailReason::ReadOnly => gettext("the disk is read-only"),
        FailReason::Other => gettext("something went wrong"),
    }
}

/// Starts the engine; called once from `controller::init`.
pub fn start(c: &mut Controller) {
    // The Trash folders are made when something is first trashed, or when
    // the Trash is opened: starting the app changes nothing (E5-11).
    let trash = Trash::new(Trash::default_home());
    // Test hook (e2e builds): slow every step down so a test can interrupt
    // a batch midway (E2-11).
    let delay = if cfg!(feature = "e2e") {
        std::env::var("CAIRN_OPS_STEP_DELAY_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .map(std::time::Duration::from_millis)
    } else {
        None
    };
    match Engine::start(Store::default_path(), trash.clone(), delay, |event| {
        later(move |c| c.on_op_event(event))
    }) {
        Ok((engine, interrupted)) => {
            c.ops.engine = Some(engine);
            c.ops.interrupted = interrupted.iter().map(|b| b.summary.clone()).collect();
            if let Some(first) = interrupted.first() {
                let text = format!(
                    "{} “{}”. {}",
                    gettext("An operation was interrupted:"),
                    first.summary,
                    gettext("Some items may be in their old place.")
                );
                c.emit(Event::Toast(text, true));
            }
        }
        Err(error) => glib::g_warning!("cairn", "could not start the operations engine: {error}"),
    }
    c.ops.trash = Some(trash);
    c.mark(dirty::OPS);
}

impl Controller {
    /// Queues a request for the engine (used by Tidy and the like).
    pub fn submit_request(&mut self, request: Request, summary: String) {
        self.submit(request, summary);
    }

    fn submit(&mut self, request: Request, summary: String) {
        let Some(engine) = self.ops.engine.as_mut() else {
            self.emit(Event::Toast(
                gettext("File operations are not available"),
                false,
            ));
            return;
        };
        let id = engine.submit(request, &summary);
        self.ops.jobs.push(JobView {
            id,
            summary,
            running: false,
            bytes: 0,
            total_bytes: 0,
            items: 0,
            total_items: 0,
            current: String::new(),
        });
        self.mark(dirty::OPS);
    }

    /// Selects the item once the folder on screen lists it.
    pub fn select_when_shown(&mut self, uri: &str) {
        self.ops.pending_select = vec![uri.to_string()];
        self.apply_pending_select();
    }

    /// Something is queued or running.
    #[cfg_attr(not(feature = "e2e"), allow(dead_code))] // Test1.WaitIdle
    pub fn ops_busy(&self) -> bool {
        !self.ops.jobs.is_empty()
    }

    fn selected_paths(&self) -> Vec<PathBuf> {
        self.selected()
            .iter()
            .filter_map(|e| e.data().file.path())
            .collect()
    }

    fn current_path(&self) -> Option<PathBuf> {
        self.current().and_then(|f| f.path())
    }

    pub fn in_trash(&self) -> bool {
        match (&self.ops.trash, self.current_path()) {
            (Some(trash), Some(path)) => path == trash.home.join("files"),
            _ => false,
        }
    }

    // --- commands ------------------------------------------------------------------

    pub fn new_folder(&mut self) {
        let Some(parent) = self.current_path() else {
            return;
        };
        if self.in_trash() {
            return;
        }
        let name = gettext("untitled folder");
        self.submit(
            Request::NewFolder {
                parent,
                name: name.clone(),
            },
            format!("{} “{name}”", gettext("New folder")),
        );
    }

    /// F2: edit the name of the selected item in place.
    pub fn start_rename(&mut self) {
        if self.in_trash() {
            return;
        }
        let selected = self.selected();
        if let [entry] = selected.as_slice() {
            self.ops.renaming = Some(entry.data().uri());
            self.mark(dirty::OPS);
            self.emit(Event::Focus("rename"));
        }
    }

    /// The in-place editor finished: an empty or unchanged name cancels.
    pub fn finish_rename(&mut self, uri: &str, name: &str) {
        if self.ops.renaming.as_deref() != Some(uri) {
            return;
        }
        self.ops.renaming = None;
        self.mark(dirty::OPS);
        self.emit(Event::Focus("content"));
        let name = name.trim();
        let Some(source) = path_of(uri) else { return };
        if name.is_empty() || name == display_name(&source) {
            return;
        }
        let summary = format!(
            "{} “{}” {} “{name}”",
            gettext("Rename"),
            display_name(&source),
            gettext("to")
        );
        self.submit(
            Request::Rename {
                source,
                name: name.to_string(),
            },
            summary,
        );
    }

    pub fn trash_selected(&mut self) {
        let sources = self.selected_paths();
        if sources.is_empty() || self.in_trash() {
            return;
        }
        let summary = format!(
            "{} {} {}",
            gettext("Move"),
            things(&sources),
            gettext("to the Trash")
        );
        self.submit(Request::Trash { sources }, summary);
    }

    pub fn duplicate(&mut self) {
        let sources = self.selected_paths();
        if sources.is_empty() || self.in_trash() {
            return;
        }
        let summary = format!("{} {}", gettext("Duplicate"), things(&sources));
        self.submit(Request::Duplicate { sources }, summary);
    }

    pub fn copy_selection(&mut self, cut: bool) {
        let paths = self.selected_paths();
        if paths.is_empty() || self.in_trash() {
            return;
        }
        let formats = clip::encode(&paths, cut);
        let (mimes, data): (Vec<String>, Vec<String>) =
            formats.into_iter().map(|(m, d)| (m.to_string(), d)).unzip();
        crate::bridge::qobject::clipboard_set(&mimes, &data);
        let text = format!(
            "{} {}",
            if cut {
                gettext("Cut")
            } else {
                gettext("Copied")
            },
            things(&paths)
        );
        self.ops.clipboard = Some(clip::Payload { paths, cut });
        self.emit(Event::Toast(text, false));
    }

    /// What Paste would bring: the system clipboard first, else our own.
    fn clipboard(&self) -> Option<clip::Payload> {
        let gnome = crate::bridge::qobject::clipboard_get(clip::GNOME_COPIED);
        let list = crate::bridge::qobject::clipboard_get(clip::URI_LIST);
        clip::decode(&gnome, &list).or_else(|| {
            let anything = !crate::bridge::qobject::clipboard_get(clip::TEXT).is_empty()
                || !crate::bridge::qobject::clipboard_get("text/plain").is_empty();
            if anything {
                None
            } else {
                self.ops.clipboard.clone()
            }
        })
    }

    pub fn paste(&mut self) {
        let Some(dest) = self.current_path() else {
            return;
        };
        if self.in_trash() {
            return;
        }
        let Some(payload) = self.clipboard() else {
            self.emit(Event::Toast(gettext("Nothing to paste"), false));
            return;
        };
        let sources: Vec<PathBuf> = payload
            .paths
            .into_iter()
            .filter(|p| clip::is_local(p))
            .collect();
        if payload.cut {
            let summary = format!(
                "{} {} {} “{}”",
                gettext("Move"),
                things(&sources),
                gettext("to"),
                display_name(&dest)
            );
            self.submit(Request::Move { sources, dest }, summary);
            // A cut is pasted once.
            self.ops.clipboard = None;
            crate::bridge::qobject::clipboard_set(&Vec::new(), &Vec::new());
        } else {
            let summary = format!(
                "{} {} {} “{}”",
                gettext("Copy"),
                things(&sources),
                gettext("to"),
                display_name(&dest)
            );
            self.submit(Request::Copy { sources, dest }, summary);
        }
    }

    /// The URIs a drag starting on `uri` carries: the selection when the
    /// item is part of it, else the item alone.
    pub fn drag_uris(&self, uri: &str) -> String {
        if self.selection.contains(uri) {
            let uris: Vec<String> = self.selected().iter().map(|e| e.data().uri()).collect();
            uris.join("\r\n")
        } else {
            uri.to_string()
        }
    }

    /// Something was dropped on `target` (a folder's URI, or "" for the
    /// folder on screen). Onto the Trash it means Move to Trash.
    pub fn dropped(&mut self, uris: &str, target: &str, ctrl: bool) {
        let sources: Vec<PathBuf> = uris.lines().map(str::trim).filter_map(path_of).collect();
        // Onto a workspace in the sidebar: add, nothing moves.
        if let Some(id) = target
            .strip_prefix("workspace:")
            .and_then(|i| i.parse().ok())
        {
            self.add_to_workspace(id, sources);
            return;
        }
        let dest = if target.is_empty() {
            self.current_path()
        } else {
            path_of(target)
        };
        let Some(dest) = dest else { return };
        if sources.is_empty() {
            return;
        }
        if Some(&dest) == self.trash_files().as_ref() {
            let summary = format!(
                "{} {} {}",
                gettext("Move"),
                things(&sources),
                gettext("to the Trash")
            );
            self.submit(Request::Trash { sources }, summary);
            return;
        }
        match cairn_core::ops::drop_request(&sources, &dest, ctrl) {
            Some(request @ Request::Copy { .. }) => {
                let summary = format!(
                    "{} {} {} “{}”",
                    gettext("Copy"),
                    things(&sources),
                    gettext("to"),
                    display_name(&dest)
                );
                self.submit(request, summary);
            }
            Some(request) => {
                let summary = format!(
                    "{} {} {} “{}”",
                    gettext("Move"),
                    things(&sources),
                    gettext("to"),
                    display_name(&dest)
                );
                self.submit(request, summary);
            }
            None => {}
        }
    }

    pub fn undo(&mut self) {
        self.submit(Request::Undo, gettext("Undo"));
    }

    pub fn redo(&mut self) {
        self.submit(Request::Redo, gettext("Redo"));
    }

    pub fn cancel_job(&mut self, id: &str) {
        let Some(engine) = &self.ops.engine else {
            return;
        };
        for job in &self.ops.jobs {
            if id.is_empty() || id == "all" || job.id.to_string() == id {
                engine.cancel(job.id);
            }
        }
    }

    /// The clash dialog's answer: "keep-both", "replace" or "skip".
    pub fn answer_clash(&mut self, choice: &str, apply_to_all: bool) {
        let Some((job, _, _)) = self.ops.clash.take() else {
            return;
        };
        let choice = match choice {
            "keep-both" => Choice::KeepBoth,
            "replace" => Choice::Replace,
            _ => Choice::Skip,
        };
        if let Some(engine) = &self.ops.engine {
            engine.answer(
                job,
                Answer {
                    choice,
                    apply_to_all,
                },
            );
        }
        if self.dialog == "clash" {
            self.close_dialog();
        }
        self.mark(dirty::OPS);
    }

    pub fn restore_selected(&mut self) {
        if !self.in_trash() {
            return;
        }
        let items = self.selected_paths();
        if items.is_empty() {
            return;
        }
        let shown: Vec<PathBuf> = items
            .iter()
            .map(|p| PathBuf::from(self.trash_name(p)))
            .collect();
        let summary = format!("{} {}", gettext("Put back"), things(&shown));
        self.submit(Request::Restore { items }, summary);
    }

    /// Empty Trash always asks first (DESIGN.md §1, rule 2).
    pub fn ask_empty_trash(&mut self) {
        let count = self.ops.trash.as_ref().map(|t| t.list().len()).unwrap_or(0);
        if count == 0 {
            self.emit(Event::Toast(gettext("The Trash is empty"), false));
            return;
        }
        self.dialog_data = json!({ "count": format::items(count as u32) });
        self.show_dialog("empty-trash");
    }

    /// Only works while the confirming dialog is open.
    pub fn confirm_empty_trash(&mut self) {
        if self.dialog != "empty-trash" {
            return;
        }
        self.close_dialog();
        self.submit(Request::EmptyTrash, gettext("Empty the Trash"));
    }

    // --- engine events ---------------------------------------------------------------

    pub fn on_op_event(&mut self, event: OpEvent) {
        match event {
            OpEvent::Started { job } => {
                if let Some(view) = self.ops.jobs.iter_mut().find(|j| j.id == job) {
                    view.running = true;
                }
            }
            OpEvent::Progress {
                job,
                bytes,
                total_bytes,
                items,
                total_items,
                current,
            } => {
                if let Some(view) = self.ops.jobs.iter_mut().find(|j| j.id == job) {
                    view.bytes = bytes;
                    view.total_bytes = total_bytes;
                    view.items = items;
                    view.total_items = total_items;
                    view.current = current;
                }
            }
            OpEvent::Clash { job, name, folder } => {
                let folder_name = display_name(&folder);
                self.ops.clash = Some((job, name.clone(), folder_name.clone()));
                self.dialog_data = json!({ "name": name, "folder": folder_name });
                self.show_dialog("clash");
            }
            OpEvent::Finished { job, report } => {
                self.ops.jobs.retain(|j| j.id != job);
                if self.ops.clash.as_ref().is_some_and(|(j, _, _)| *j == job) {
                    self.ops.clash = None;
                    if self.dialog == "clash" {
                        self.close_dialog();
                    }
                }
                self.finished(report);
            }
        }
        self.mark(dirty::OPS);
    }

    fn finished(&mut self, report: Report) {
        let mut touched: Vec<PathBuf> = cairn_core::ops::touched_folders(&report)
            .into_iter()
            .collect();
        touched.extend(self.current_path());
        super::index::notify(&touched);
        self.ops.last_report = json!({
            "kind": report.kind,
            "outcome": match report.outcome { Outcome::Forward => "forward", Outcome::Undo => "undo", Outcome::Redo => "redo" },
            "done": report.done,
            "skipped": report.skipped,
            "cancelled": report.cancelled,
            "batch": report.batch,
            "failed": report.failed.iter().map(|f| json!({
                "path": f.path.display().to_string(), "reason": format!("{:?}", f.reason),
            })).collect::<Vec<_>>(),
        });
        // Select what the operation put in this folder.
        let here = self.current_path();
        let created: Vec<String> = report
            .created
            .iter()
            .filter(|p| p.parent() == here.as_deref())
            .map(|p| uri_of(p))
            .collect();
        if report.kind == "new-folder" && report.outcome == Outcome::Forward {
            self.ops.rename_when_shown = created.first().cloned();
        }
        if !created.is_empty() {
            self.ops.pending_select = created;
            self.apply_pending_select();
        }
        let clean_text = self.clean_report_text(&report);
        self.clean_after_op(report.outcome == Outcome::Undo);
        if let Some(text) = clean_text.or_else(|| self.report_text(&report)) {
            let undo = report.outcome != Outcome::Undo
                && report.done > 0
                && report.kind != "empty-trash"
                && report.batch.is_some();
            self.emit(Event::Toast(text, undo));
        }
        if report.kind == "empty-trash" || self.in_trash() {
            self.reload();
        }
        self.refresh_suggestions();
        self.refresh_git();
    }

    /// The toast for a finished report (None: nothing worth saying).
    fn report_text(&self, report: &Report) -> Option<String> {
        let failures = |verb: String| -> Option<String> {
            let first = report.failed.first()?;
            Some(format!(
                "{verb} {}: “{}” ({})",
                format::items(report.failed.len() as u32),
                display_name(&first.path),
                reason(first.reason)
            ))
        };
        match report.outcome {
            Outcome::Undo | Outcome::Redo if report.done == 0 && report.failed.is_empty() => {
                Some(if report.outcome == Outcome::Undo {
                    gettext("Nothing to undo")
                } else {
                    gettext("Nothing to redo")
                })
            }
            Outcome::Undo if !report.failed.is_empty() => failures(gettext("Could not put back")),
            Outcome::Undo => Some(format!("{} {}", gettext("Undone:"), report.summary)),
            Outcome::Redo if !report.failed.is_empty() => failures(gettext("Could not redo")),
            Outcome::Redo => Some(format!("{} {}", gettext("Redone:"), report.summary)),
            Outcome::Forward => {
                if !report.failed.is_empty() {
                    return failures(gettext("Could not do it for"));
                }
                if report.cancelled {
                    return Some(gettext("Cancelled"));
                }
                if report.done == 0 {
                    return (report.skipped > 0).then(|| gettext("Nothing changed"));
                }
                let n = format::items(report.done as u32);
                Some(match report.kind.as_str() {
                    "trash" => format!("{} {n} {}", gettext("Moved"), gettext("to the Trash")),
                    "copy" => format!("{} {n}", gettext("Copied")),
                    "move" => format!("{} {n}", gettext("Moved")),
                    "duplicate" => format!("{} {n}", gettext("Duplicated")),
                    "restore" => format!("{} {n}", gettext("Put back")),
                    "rename" => format!(
                        "{} “{}”",
                        gettext("Renamed to"),
                        report
                            .created
                            .first()
                            .map(|p| display_name(p))
                            .unwrap_or_default()
                    ),
                    "empty-trash" => gettext("Emptied the Trash"),
                    "tidy" => {
                        let mut folders: Vec<_> =
                            report.created.iter().filter_map(|p| p.parent()).collect();
                        folders.sort();
                        folders.dedup();
                        format!(
                            "{} {} {} {}",
                            gettext("Tidied"),
                            format::files(report.done as u32),
                            gettext("into"),
                            format::folders(folders.len() as u32)
                        )
                    }
                    _ => return None,
                })
            }
        }
    }

    /// After a recompute: select what an operation just made, and start
    /// renaming a new folder.
    pub fn apply_pending_select(&mut self) {
        if self.ops.pending_select.is_empty() {
            return;
        }
        let shown: Vec<String> = self
            .visible
            .iter()
            .map(|e| e.data().uri())
            .filter(|u| self.ops.pending_select.contains(u))
            .collect();
        if shown.len() < self.ops.pending_select.len() {
            return; // not all listed yet; the next recompute tries again
        }
        self.ops.pending_select.clear();
        self.selection = shown.into_iter().collect();
        self.anchor = self
            .visible
            .iter()
            .position(|e| self.selection.contains(&e.data().uri()));
        self.mark(dirty::SELECTION | dirty::PREVIEW | dirty::FOLDER);
        if let Some(uri) = self.ops.rename_when_shown.take()
            && self.selection.contains(&uri)
        {
            self.ops.renaming = Some(uri);
            self.mark(dirty::OPS);
            self.emit(Event::Focus("rename"));
        }
    }

    // --- the Trash view ----------------------------------------------------------------

    /// The original name of an item in the Trash (its name there may be "a.txt.2").
    pub fn trash_name(&self, item: &Path) -> String {
        self.ops
            .trash
            .as_ref()
            .and_then(|t| t.original_of(item))
            .map(|p| display_name(&p))
            .unwrap_or_else(|| display_name(item))
    }

    pub fn trash_origin(&self, item: &Path) -> Option<String> {
        let original = self.ops.trash.as_ref()?.original_of(item)?;
        Some(paths::display_path(original.parent()?))
    }

    pub fn trash_files(&self) -> Option<PathBuf> {
        self.ops.trash.as_ref().map(|t| t.home.join("files"))
    }

    // --- what QML shows ---------------------------------------------------------------

    fn undo_labels(&self) -> (Option<String>, Option<String>) {
        let store = app_data::store();
        let store = store.borrow();
        (
            store.undo_candidate().ok().flatten().map(|b| b.summary),
            store.redo_candidate().ok().flatten().map(|b| b.summary),
        )
    }

    pub fn ops_json(&self) -> Value {
        let (undo, redo) = self.undo_labels();
        let total_bytes: u64 = self.ops.jobs.iter().map(|j| j.total_bytes).sum();
        let bytes: u64 = self.ops.jobs.iter().map(|j| j.bytes).sum();
        json!({
            "jobs": self.ops.jobs.iter().map(|j| json!({
                "id": j.id.to_string(), "summary": j.summary, "running": j.running,
                "bytes": j.bytes, "totalBytes": j.total_bytes, "items": j.items, "totalItems": j.total_items,
                "current": j.current,
                "fraction": if j.total_bytes > 0 { j.bytes as f64 / j.total_bytes as f64 } else { 0.0 },
                "progress": if j.total_bytes > 0 {
                    format!("{} {} {}", format::size(j.bytes), gettext("of"), format::size(j.total_bytes))
                } else { String::new() },
            })).collect::<Vec<_>>(),
            "fraction": if total_bytes > 0 { bytes as f64 / total_bytes as f64 } else { 0.0 },
            "undo": undo,
            "redo": redo,
            "renaming": self.ops.renaming,
            "trash": self.in_trash(),
            "trashFolder": self.trash_files().map(|p| p.display().to_string()),
            "canPaste": self.ops.clipboard.is_some(),
        })
    }

    #[cfg(feature = "e2e")]
    pub fn ops_test_json(&self) -> Value {
        let store = app_data::store();
        let store = store.borrow();
        let mut state = self.ops_json();
        state["clash"] = match &self.ops.clash {
            Some((_, name, folder)) => json!({ "name": name, "folder": folder }),
            None => Value::Null,
        };
        state["last_report"] = self.ops.last_report.clone();
        state["interrupted"] = json!(self.ops.interrupted);
        state["journal"] = json!(store.batches().unwrap_or_default().iter().map(|b| json!({
            "id": b.id, "kind": b.kind, "summary": b.summary, "state": b.state, "undoable": b.undoable,
        })).collect::<Vec<_>>());
        state
    }
}
