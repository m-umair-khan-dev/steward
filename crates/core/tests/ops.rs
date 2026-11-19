//! L2 tests for the operations engine (P2-T01, P2-T02) on a real filesystem.

use std::collections::VecDeque;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use gio::prelude::*;

use cairn_core::ops::{
    Answer, Choice, Context, FailReason, Outcome, Report, Request, Trash, Worker,
};
use cairn_core::store::Store;

struct Place {
    root: PathBuf,
}

impl Place {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("cairn-ops-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("home")).unwrap();
        Self { root }
    }

    fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.home().join(rel)
    }

    fn write(&self, rel: &str, text: &str) -> PathBuf {
        let path = self.path(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }

    fn worker(&self) -> Worker {
        Worker::new(
            Store::open(&self.root.join("app.db")).unwrap(),
            Trash::new(self.root.join("home/.local/share/Trash")),
        )
    }

    fn trash(&self) -> Trash {
        Trash::new(self.root.join("home/.local/share/Trash"))
    }

    /// Every path under home (except the Trash) with its contents.
    fn snapshot(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        walk(&self.home(), &self.home(), &mut out);
        out.sort();
        out
    }
}

fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        let rel = path.strip_prefix(base).unwrap().display().to_string();
        if rel.starts_with(".local") {
            continue;
        }
        let meta = path.symlink_metadata().unwrap();
        if meta.file_type().is_symlink() {
            out.push((
                rel,
                format!("-> {}", std::fs::read_link(&path).unwrap().display()),
            ));
        } else if meta.is_dir() {
            out.push((rel.clone(), "/".into()));
            walk(base, &path, out);
        } else {
            out.push((rel, std::fs::read_to_string(&path).unwrap_or_default()));
        }
    }
}

impl Drop for Place {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[derive(Default)]
struct Ctx {
    answers: VecDeque<Answer>,
    asked: Vec<String>,
    cancel: gio::Cancellable,
    cancel_after_bytes: Option<u64>,
    progress_calls: usize,
}

impl Context for Ctx {
    fn progress(
        &mut self,
        bytes: u64,
        _total: u64,
        _items: u64,
        _total_items: u64,
        _current: &str,
    ) {
        self.progress_calls += 1;
        if self.cancel_after_bytes.is_some_and(|limit| bytes >= limit) {
            self.cancel.cancel();
        }
    }
    fn ask(&mut self, name: &str, _folder: &Path) -> Answer {
        self.asked.push(name.to_string());
        self.answers.pop_front().expect("an answer for the clash")
    }
    fn cancellable(&self) -> &gio::Cancellable {
        &self.cancel
    }
}

fn run(worker: &Worker, request: Request) -> Report {
    worker.run(&request, "test", &mut Ctx::default())
}

fn answer(choice: Choice) -> Answer {
    Answer {
        choice,
        apply_to_all: false,
    }
}

fn state(worker: &Worker, batch: i64) -> String {
    worker.store.batch(batch).unwrap().unwrap().state
}

// --- each operation -------------------------------------------------------------

#[test]
fn new_folder_and_rename_then_undo_both() {
    let place = Place::new("e2-01");
    let file = place.write("a.txt", "A");
    let worker = place.worker();
    let before = place.snapshot();

    let made = run(
        &worker,
        Request::NewFolder {
            parent: place.home(),
            name: "untitled folder".into(),
        },
    );
    assert_eq!(made.created, vec![place.path("untitled folder")]);
    assert!(place.path("untitled folder").is_dir());
    // A second one gets a free name.
    let again = run(
        &worker,
        Request::NewFolder {
            parent: place.home(),
            name: "untitled folder".into(),
        },
    );
    assert_eq!(again.created, vec![place.path("untitled folder (2)")]);

    run(
        &worker,
        Request::Rename {
            source: file,
            name: "b.txt".into(),
        },
    );
    assert!(place.path("b.txt").exists() && !place.path("a.txt").exists());

    for _ in 0..3 {
        let undo = run(&worker, Request::Undo);
        assert_eq!(undo.outcome, Outcome::Undo);
        assert!(undo.failed.is_empty(), "{:?}", undo.failed);
    }
    assert_eq!(place.snapshot(), before);
}

#[test]
fn rename_refuses_a_taken_name_and_bad_names() {
    let place = Place::new("rename-taken");
    let a = place.write("a.txt", "A");
    place.write("b.txt", "B");
    let worker = place.worker();
    let report = run(
        &worker,
        Request::Rename {
            source: a.clone(),
            name: "b.txt".into(),
        },
    );
    assert_eq!(report.failed[0].reason, FailReason::NameTaken);
    assert_eq!(std::fs::read_to_string(place.path("b.txt")).unwrap(), "B");
    let bad = run(
        &worker,
        Request::Rename {
            source: a,
            name: "../x".into(),
        },
    );
    assert_eq!(bad.failed.len(), 1);
    assert!(
        worker.store.batches().unwrap().is_empty(),
        "failed batches are not kept"
    );
}

#[test]
fn trash_three_files_then_undo_restores_paths_and_times() {
    let place = Place::new("e2-02");
    let worker = place.worker();
    let files: Vec<PathBuf> = ["x.txt", "y.txt", "sub/z.txt"]
        .iter()
        .map(|n| place.write(n, n))
        .collect();
    let old = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    for f in &files {
        std::fs::File::options()
            .write(true)
            .open(f)
            .unwrap()
            .set_modified(old)
            .unwrap();
    }
    let before = place.snapshot();

    let report = run(
        &worker,
        Request::Trash {
            sources: files.clone(),
        },
    );
    assert_eq!(report.done, 3);
    assert!(files.iter().all(|f| !f.exists()));
    let items = place.trash().list();
    assert_eq!(items.len(), 3);
    assert!(items.iter().all(|i| files.contains(&i.original)));

    let undo = run(&worker, Request::Undo);
    assert!(undo.failed.is_empty());
    assert_eq!(place.snapshot(), before);
    for f in &files {
        assert_eq!(
            f.metadata().unwrap().modified().unwrap(),
            old,
            "{f:?} keeps its time"
        );
    }
    assert!(place.trash().list().is_empty());
}

#[test]
fn trashing_the_same_name_twice_keeps_both_in_the_trash() {
    let place = Place::new("trash-twice");
    let worker = place.worker();
    run(
        &worker,
        Request::Trash {
            sources: vec![place.write("a.txt", "1")],
        },
    );
    run(
        &worker,
        Request::Trash {
            sources: vec![place.write("a.txt", "2")],
        },
    );
    let items = place.trash().list();
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|i| i.original == place.path("a.txt")));
    // Undo puts back the second; the first cannot come back over it.
    run(&worker, Request::Undo);
    assert_eq!(std::fs::read_to_string(place.path("a.txt")).unwrap(), "2");
    let blocked = run(&worker, Request::Undo);
    assert_eq!(blocked.failed[0].reason, FailReason::NameTaken);
    assert_eq!(std::fs::read_to_string(place.path("a.txt")).unwrap(), "2");
}

#[test]
fn copy_two_files_then_undo_removes_only_the_copies() {
    let place = Place::new("e2-03");
    let worker = place.worker();
    let a = place.write("src/a.txt", "A");
    let b = place.write("src/b.txt", "B");
    std::fs::create_dir(place.path("dst")).unwrap();
    place.write("dst/keep.txt", "K");
    let before = place.snapshot();

    let report = run(
        &worker,
        Request::Copy {
            sources: vec![a.clone(), b.clone()],
            dest: place.path("dst"),
        },
    );
    assert_eq!(report.done, 2);
    assert_eq!(
        std::fs::read_to_string(place.path("dst/a.txt")).unwrap(),
        "A"
    );
    assert!(a.exists() && b.exists());

    run(&worker, Request::Undo);
    assert_eq!(place.snapshot(), before);
    // Redo makes the copies again.
    let redo = run(&worker, Request::Redo);
    assert_eq!(redo.outcome, Outcome::Redo);
    assert!(place.path("dst/a.txt").exists() && place.path("dst/b.txt").exists());
}

#[test]
fn copying_a_folder_copies_everything_and_links_as_links() {
    let place = Place::new("copy-tree");
    let worker = place.worker();
    place.write("tree/one.txt", "1");
    place.write("tree/deep/two.txt", "2");
    std::os::unix::fs::symlink("one.txt", place.path("tree/link")).unwrap();
    std::fs::create_dir(place.path("out")).unwrap();
    run(
        &worker,
        Request::Copy {
            sources: vec![place.path("tree")],
            dest: place.path("out"),
        },
    );
    assert_eq!(
        std::fs::read_to_string(place.path("out/tree/deep/two.txt")).unwrap(),
        "2"
    );
    let link = place.path("out/tree/link");
    assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
    assert_eq!(std::fs::read_link(link).unwrap(), Path::new("one.txt"));
}

#[test]
fn cut_and_paste_then_undo_moves_back() {
    let place = Place::new("e2-04");
    let worker = place.worker();
    let a = place.write("src/a.txt", "A");
    let dir = place.write("src/folder/inner.txt", "I");
    std::fs::create_dir(place.path("dst")).unwrap();
    let before = place.snapshot();
    let inode = a.metadata().unwrap().ino();

    run(
        &worker,
        Request::Move {
            sources: vec![a.clone(), dir.parent().unwrap().into()],
            dest: place.path("dst"),
        },
    );
    assert!(!a.exists() && place.path("dst/a.txt").exists());
    assert_eq!(
        place.path("dst/a.txt").metadata().unwrap().ino(),
        inode,
        "a rename, not a copy"
    );
    assert!(place.path("dst/folder/inner.txt").exists());

    run(&worker, Request::Undo);
    assert_eq!(place.snapshot(), before);
}

#[test]
fn a_folder_cannot_move_into_itself() {
    let place = Place::new("into-itself");
    let worker = place.worker();
    place.write("f/inner/x.txt", "x");
    let report = run(
        &worker,
        Request::Move {
            sources: vec![place.path("f")],
            dest: place.path("f/inner"),
        },
    );
    assert_eq!(report.failed.len(), 1);
    assert!(place.path("f/inner/x.txt").exists());
}

#[test]
fn duplicate_keeps_both() {
    let place = Place::new("duplicate");
    let worker = place.worker();
    let a = place.write("a.txt", "A");
    let report = run(&worker, Request::Duplicate { sources: vec![a] });
    assert_eq!(report.created, vec![place.path("a (2).txt")]);
    // Pasting a copy into the same folder also keeps both, without asking.
    let mut ctx = Ctx::default();
    worker.run(
        &Request::Copy {
            sources: vec![place.path("a.txt")],
            dest: place.home(),
        },
        "t",
        &mut ctx,
    );
    assert!(ctx.asked.is_empty());
    assert!(place.path("a (3).txt").exists());
}

// --- clashes (E2-05) ---------------------------------------------------------------

#[test]
fn clash_keep_both_replace_and_skip() {
    let place = Place::new("e2-05");
    let worker = place.worker();
    let src = place.write("src/report.pdf", "new");
    place.write("dst/report.pdf", "old");
    let dest = place.path("dst");

    let mut ctx = Ctx {
        answers: [answer(Choice::Skip)].into(),
        ..Default::default()
    };
    let skip = worker.run(
        &Request::Copy {
            sources: vec![src.clone()],
            dest: dest.clone(),
        },
        "t",
        &mut ctx,
    );
    assert_eq!((skip.done, skip.skipped), (0, 1));
    assert_eq!(
        std::fs::read_to_string(dest.join("report.pdf")).unwrap(),
        "old"
    );

    let mut ctx = Ctx {
        answers: [answer(Choice::KeepBoth)].into(),
        ..Default::default()
    };
    worker.run(
        &Request::Copy {
            sources: vec![src.clone()],
            dest: dest.clone(),
        },
        "t",
        &mut ctx,
    );
    assert_eq!(
        std::fs::read_to_string(dest.join("report (2).pdf")).unwrap(),
        "new"
    );
    assert_eq!(
        std::fs::read_to_string(dest.join("report.pdf")).unwrap(),
        "old"
    );

    let mut ctx = Ctx {
        answers: [answer(Choice::Replace)].into(),
        ..Default::default()
    };
    worker.run(
        &Request::Copy {
            sources: vec![src.clone()],
            dest: dest.clone(),
        },
        "t",
        &mut ctx,
    );
    assert_eq!(
        std::fs::read_to_string(dest.join("report.pdf")).unwrap(),
        "new"
    );
    let trashed = place.trash().list();
    assert_eq!(trashed.len(), 1, "the replaced file went to the Trash");

    // Undo of Replace: the new copy leaves, the old file comes back.
    run(&worker, Request::Undo);
    assert_eq!(
        std::fs::read_to_string(dest.join("report.pdf")).unwrap(),
        "old"
    );
}

#[test]
fn apply_to_all_asks_once() {
    let place = Place::new("apply-all");
    let worker = place.worker();
    let sources: Vec<PathBuf> = (0..3)
        .map(|i| place.write(&format!("src/{i}.txt"), "n"))
        .collect();
    for i in 0..3 {
        place.write(&format!("dst/{i}.txt"), "o");
    }
    let mut ctx = Ctx {
        answers: [Answer {
            choice: Choice::Skip,
            apply_to_all: true,
        }]
        .into(),
        ..Default::default()
    };
    let report = worker.run(
        &Request::Copy {
            sources,
            dest: place.path("dst"),
        },
        "t",
        &mut ctx,
    );
    assert_eq!(ctx.asked.len(), 1);
    assert_eq!(report.skipped, 3);
}

// --- undo rules (E2-06 and friends) -------------------------------------------------

#[test]
fn undo_never_overwrites_and_ends_partial() {
    let place = Place::new("e2-06");
    let worker = place.worker();
    let a = place.write("a.txt", "mine");
    let b = place.write("b.txt", "b");
    std::fs::create_dir(place.path("dst")).unwrap();
    let report = run(
        &worker,
        Request::Move {
            sources: vec![a.clone(), b.clone()],
            dest: place.path("dst"),
        },
    );
    let batch = report.batch.unwrap();
    std::fs::write(&a, "someone else's").unwrap();

    let undo = run(&worker, Request::Undo);
    assert_eq!(undo.failed.len(), 1);
    assert_eq!(undo.failed[0].path, a);
    assert_eq!(undo.failed[0].reason, FailReason::NameTaken);
    assert_eq!(
        std::fs::read_to_string(&a).unwrap(),
        "someone else's",
        "nothing overwritten"
    );
    assert_eq!(
        std::fs::read_to_string(place.path("dst/a.txt")).unwrap(),
        "mine",
        "left where it was"
    );
    assert!(b.exists(), "the other file came back");
    assert_eq!(state(&worker, batch), "partial");
}

#[test]
fn undo_reports_items_that_are_gone() {
    let place = Place::new("gone");
    let worker = place.worker();
    let a = place.write("a.txt", "A");
    run(
        &worker,
        Request::Rename {
            source: a,
            name: "b.txt".into(),
        },
    );
    std::fs::remove_file(place.path("b.txt")).unwrap();
    let undo = run(&worker, Request::Undo);
    assert_eq!(undo.failed[0].reason, FailReason::Gone);
}

#[test]
fn redo_chain_and_a_new_action_ends_it() {
    let place = Place::new("redo");
    let worker = place.worker();
    let a = place.write("a.txt", "A");
    run(
        &worker,
        Request::Rename {
            source: a,
            name: "b.txt".into(),
        },
    );
    run(
        &worker,
        Request::Rename {
            source: place.path("b.txt"),
            name: "c.txt".into(),
        },
    );
    run(&worker, Request::Undo);
    run(&worker, Request::Undo);
    assert!(place.path("a.txt").exists());
    run(&worker, Request::Redo);
    assert!(
        place.path("b.txt").exists(),
        "redo repeats the oldest undone first"
    );
    run(&worker, Request::Redo);
    assert!(place.path("c.txt").exists());
    run(&worker, Request::Undo);
    run(
        &worker,
        Request::NewFolder {
            parent: place.home(),
            name: "n".into(),
        },
    );
    let none = run(&worker, Request::Redo);
    assert_eq!(none.done, 0, "a new action ends the redo chain");
    assert!(place.path("b.txt").exists());
}

#[test]
fn undo_survives_reopening_the_journal() {
    let place = Place::new("e2-12");
    let a = place.write("a.txt", "A");
    std::fs::create_dir(place.path("dst")).unwrap();
    run(
        &place.worker(),
        Request::Move {
            sources: vec![a.clone()],
            dest: place.path("dst"),
        },
    );
    let reopened = place.worker();
    run(&reopened, Request::Undo);
    assert!(a.exists());
}

#[test]
fn the_journal_keeps_fifty_batches() {
    let place = Place::new("keep");
    let worker = place.worker();
    for i in 0..55 {
        run(
            &worker,
            Request::NewFolder {
                parent: place.home(),
                name: format!("f{i}"),
            },
        );
    }
    assert_eq!(worker.store.batches().unwrap().len(), 50);
}

// --- trash view (E2-09) --------------------------------------------------------------

#[test]
fn restore_from_the_trash_and_empty_it() {
    let place = Place::new("e2-09");
    let worker = place.worker();
    let a = place.write("docs/a.txt", "A");
    let b = place.write("b.txt", "B");
    run(
        &worker,
        Request::Trash {
            sources: vec![a.clone()],
        },
    );
    run(
        &worker,
        Request::Trash {
            sources: vec![b.clone()],
        },
    );

    let item = place
        .trash()
        .list()
        .into_iter()
        .find(|i| i.original == a)
        .unwrap();
    run(
        &worker,
        Request::Restore {
            items: vec![item.path],
        },
    );
    assert_eq!(std::fs::read_to_string(&a).unwrap(), "A");

    let emptied = run(&worker, Request::EmptyTrash);
    assert_eq!(emptied.done, 1);
    assert!(place.trash().list().is_empty());
    // Undo now skips the batch whose item is gone for good: the next
    // candidate is the restore, which sends a.txt back to the Trash.
    let undo = run(&worker, Request::Undo);
    assert_eq!(undo.kind, "restore");
    assert!(!a.exists() && !b.exists());
}

// --- cancel, failures, crash ------------------------------------------------------------

#[test]
fn cancelling_a_copy_leaves_no_partial_file() {
    let place = Place::new("e2-07");
    let worker = place.worker();
    let big = place.path("big.iso");
    let file = std::fs::File::create(&big).unwrap();
    file.set_len(64 * 1024 * 1024).unwrap(); // sparse
    std::fs::create_dir(place.path("dst")).unwrap();
    let mut ctx = Ctx {
        cancel_after_bytes: Some(1),
        ..Default::default()
    };
    let report = worker.run(
        &Request::Copy {
            sources: vec![big],
            dest: place.path("dst"),
        },
        "t",
        &mut ctx,
    );
    assert!(report.cancelled);
    assert!(ctx.progress_calls > 0);
    assert!(!place.path("dst/big.iso").exists(), "no partial file");
    assert!(
        worker.store.batches().unwrap().is_empty(),
        "nothing done, nothing journaled"
    );
}

#[test]
fn paste_into_a_read_only_folder_changes_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let place = Place::new("e2-10");
    let worker = place.worker();
    let a = place.write("a.txt", "A");
    let locked = place.path("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::write(locked.join("probe"), "").is_ok() {
        return; // running as root: permissions are not enforced
    }
    let before = place.snapshot();
    let report = run(
        &worker,
        Request::Copy {
            sources: vec![a.clone()],
            dest: locked.clone(),
        },
    );
    assert_eq!(report.failed[0].reason, FailReason::PermissionDenied);
    let moved = run(
        &worker,
        Request::Move {
            sources: vec![a],
            dest: locked.clone(),
        },
    );
    assert_eq!(moved.failed.len(), 1);
    assert_eq!(place.snapshot(), before);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn trashing_a_link_to_a_folder_keeps_the_folder() {
    let place = Place::new("e2-14");
    let worker = place.worker();
    place.write("target/inside.txt", "keep");
    let link = place.path("link");
    std::os::unix::fs::symlink(place.path("target"), &link).unwrap();
    run(
        &worker,
        Request::Trash {
            sources: vec![link.clone()],
        },
    );
    assert!(link.symlink_metadata().is_err());
    assert_eq!(
        std::fs::read_to_string(place.path("target/inside.txt")).unwrap(),
        "keep"
    );
    run(&worker, Request::EmptyTrash);
    assert_eq!(
        std::fs::read_to_string(place.path("target/inside.txt")).unwrap(),
        "keep"
    );
}

#[test]
fn a_batch_left_running_is_recovered_as_partial() {
    let place = Place::new("e2-11");
    let store = Store::open(&place.root.join("app.db")).unwrap();
    use cairn_core::ops::journal::Action;
    let a = place.write("dst/a.txt", "moved before the crash");
    let b = place.write("src/b.txt", "not moved");
    let batch = store.begin_batch("move", "Move 2 items").unwrap();
    store
        .add_step(
            batch,
            1,
            Action::Move,
            Some(&place.path("src/a.txt")),
            Some(&a),
        )
        .unwrap();
    store
        .add_step(
            batch,
            2,
            Action::Move,
            Some(&b),
            Some(&place.path("dst/b.txt")),
        )
        .unwrap();
    drop(store);

    let store = Store::open(&place.root.join("app.db")).unwrap();
    let open = store.recover().unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(store.batch(batch).unwrap().unwrap().state, "partial");
    let steps = store.steps(batch).unwrap();
    assert_eq!(steps[0].state, "done");
    assert_eq!(steps[1].state, "failed");
    // Undo puts back what moved.
    let worker = Worker::new(store, place.trash());
    run(&worker, Request::Undo);
    assert!(place.path("src/a.txt").exists() && b.exists());
}

#[test]
fn copies_keep_holes_times_and_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let place = Place::new("sparse");
    let worker = place.worker();
    let src = place.path("disk.img");
    {
        use std::io::{Seek, SeekFrom, Write};
        let mut f = std::fs::File::create(&src).unwrap();
        f.write_all(b"head").unwrap();
        f.seek(SeekFrom::Start(256 * 1024 * 1024)).unwrap();
        f.write_all(b"tail").unwrap();
    }
    std::fs::set_permissions(&src, std::fs::Permissions::from_mode(0o640)).unwrap();
    let old = SystemTime::UNIX_EPOCH + Duration::from_secs(1_600_000_000);
    std::fs::File::options()
        .write(true)
        .open(&src)
        .unwrap()
        .set_modified(old)
        .unwrap();
    std::fs::create_dir(place.path("out")).unwrap();
    run(
        &worker,
        Request::Copy {
            sources: vec![src.clone()],
            dest: place.path("out"),
        },
    );
    let copy = place.path("out/disk.img");
    let (a, b) = (src.metadata().unwrap(), copy.metadata().unwrap());
    assert_eq!(a.len(), b.len());
    assert!(
        b.blocks() * 512 < 16 * 1024 * 1024,
        "the hole stayed a hole ({} blocks)",
        b.blocks()
    );
    assert_eq!(b.modified().unwrap(), old);
    assert_eq!(b.permissions().mode() & 0o777, 0o640);
    let bytes = std::fs::read(&copy).unwrap();
    assert_eq!(&bytes[..4], b"head");
    assert_eq!(&bytes[bytes.len() - 4..], b"tail");
}

#[test]
fn drops_move_by_default_and_copy_with_ctrl() {
    use cairn_core::ops::drop_request;
    let place = Place::new("drop");
    let a = place.write("src/a.txt", "A");
    let folder = place.path("src");
    let dest = place.path("dst");
    std::fs::create_dir(&dest).unwrap();
    assert_eq!(
        drop_request(std::slice::from_ref(&a), &dest, false),
        Some(Request::Move {
            sources: vec![a.clone()],
            dest: dest.clone()
        })
    );
    assert_eq!(
        drop_request(std::slice::from_ref(&a), &dest, true),
        Some(Request::Copy {
            sources: vec![a.clone()],
            dest: dest.clone()
        })
    );
    // Onto its own folder, or a folder into itself: nothing to do.
    assert_eq!(drop_request(std::slice::from_ref(&a), &folder, false), None);
    assert_eq!(
        drop_request(std::slice::from_ref(&folder), &folder.join("inner"), false),
        None
    );
    // Across filesystems (/proc is never the same device): a copy.
    let elsewhere = Path::new("/proc");
    assert!(matches!(
        drop_request(std::slice::from_ref(&a), elsewhere, false),
        Some(Request::Copy { .. })
    ));
}
