//! L2 tests: the folder model against a real fixture home (P1-T01, P1-T02, P1-T10).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use cairn_core::{FileEntry, FolderModel, Kind, LoadState};
use gio::prelude::*;

fn fixture_home(name: &str) -> (PathBuf, serde_json::Value) {
    let root = std::env::temp_dir().join(format!("cairn-l2-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docker/scripts/make-fixture-home");
    let status = Command::new("python3")
        .arg("-I")
        .arg(script)
        .arg(&root)
        .status()
        .unwrap();
    assert!(status.success(), "fixture generator failed");
    let manifest = std::fs::read_to_string(root.join(".cairn-fixture")).unwrap();
    (root, serde_json::from_str(&manifest).unwrap())
}

/// Runs the model's async work on a private main context until `done` holds.
fn run_until(ctx: &glib::MainContext, timeout: Duration, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + timeout;
    while !done() {
        assert!(Instant::now() < deadline, "timed out");
        ctx.iteration(false);
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn names(model: &FolderModel) -> Vec<String> {
    let store = model.store();
    let mut names: Vec<String> = (0..store.n_items())
        .map(|i| {
            store
                .item(i)
                .and_downcast::<FileEntry>()
                .unwrap()
                .name()
                .to_string()
        })
        .collect();
    names.sort();
    names
}

fn load(ctx: &glib::MainContext, path: &Path) -> FolderModel {
    let model = FolderModel::new();
    model.load(&gio::File::for_path(path));
    run_until(ctx, Duration::from_secs(10), || {
        model.state() != LoadState::Loading
    });
    model
}

#[test]
fn downloads_listing_matches_the_manifest() {
    let (home, manifest) = fixture_home("listing");
    let ctx = glib::MainContext::new();
    ctx.with_thread_default(|| {
        let model = load(&ctx, &home.join("Downloads"));
        assert_eq!(model.state(), LoadState::Ready);
        let facts = &manifest["folders"]["Downloads"];
        assert_eq!(
            model.store().n_items() as u64,
            facts["items"].as_u64().unwrap()
        );

        let mut expected: Vec<String> = manifest["downloads"]["loose_files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        expected.extend(["archive", "Receipts", "Wallpapers"].map(String::from));
        expected.sort();
        assert_eq!(names(&model), expected);
    })
    .unwrap();
}

#[test]
fn kinds_agree_with_the_fixture_generator() {
    let (home, manifest) = fixture_home("kinds");
    let fixture_kind_ids = |label: &str| match label {
        "PDFs" => "pdfs".to_string(),
        other => other.to_lowercase(),
    };
    let ctx = glib::MainContext::new();
    ctx.with_thread_default(|| {
        let model = load(&ctx, &home.join("Downloads"));
        let store = model.store();
        let kind_of = |name: &str| {
            (0..store.n_items())
                .map(|i| store.item(i).and_downcast::<FileEntry>().unwrap())
                .find(|e| e.name() == name)
                .unwrap_or_else(|| panic!("{name} not listed"))
                .kind()
        };
        for (label, files) in manifest["downloads"]["by_kind"].as_object().unwrap() {
            for file in files.as_array().unwrap() {
                let file = file.as_str().unwrap();
                assert_eq!(kind_of(file).id(), fixture_kind_ids(label), "{file}");
            }
        }
        for partial in manifest["downloads"]["partial"].as_array().unwrap() {
            assert_eq!(kind_of(partial.as_str().unwrap()), Kind::Other);
        }
        assert_eq!(kind_of("Receipts"), Kind::Folder);
    })
    .unwrap();
}

#[test]
fn listing_states_for_missing_and_unreadable_folders() {
    let (home, _) = fixture_home("states");
    let ctx = glib::MainContext::new();
    ctx.with_thread_default(|| {
        assert_eq!(
            load(&ctx, &home.join("no-such-folder")).state(),
            LoadState::NotFound
        );
        assert_eq!(
            load(&ctx, &home.join("Desktop/todo.txt")).state(),
            LoadState::NotAFolder
        );

        let locked = home.join("locked");
        std::fs::create_dir(&locked).unwrap();
        std::fs::set_permissions(&locked, std::os::unix::fs::PermissionsExt::from_mode(0o000))
            .unwrap();
        let state = load(&ctx, &locked).state();
        std::fs::set_permissions(&locked, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .unwrap();
        assert_eq!(state, LoadState::PermissionDenied);
    })
    .unwrap();
}

#[test]
fn the_listing_follows_changes_on_disk() {
    let (home, _) = fixture_home("monitor");
    let folder = home.join("Desktop");
    let ctx = glib::MainContext::new();
    ctx.with_thread_default(|| {
        let model = load(&ctx, &folder);
        assert_eq!(names(&model), vec!["todo.txt"]);

        std::fs::write(folder.join("new.txt"), "hello").unwrap();
        run_until(&ctx, Duration::from_secs(5), || {
            names(&model).contains(&"new.txt".into())
        });

        std::fs::rename(folder.join("new.txt"), folder.join("renamed.txt")).unwrap();
        run_until(&ctx, Duration::from_secs(5), || {
            names(&model) == vec!["renamed.txt".to_string(), "todo.txt".to_string()]
        });

        std::fs::remove_file(folder.join("renamed.txt")).unwrap();
        run_until(&ctx, Duration::from_secs(5), || {
            names(&model) == vec!["todo.txt"]
        });
    })
    .unwrap();
}
