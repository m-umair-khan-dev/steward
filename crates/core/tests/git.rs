//! L1/L2 for `core::git` (P7-T01): the fixture's Git project.

use std::path::{Path, PathBuf};
use std::process::Command;

use cairn_core::git;

fn fixture(name: &str) -> (PathBuf, serde_json::Value) {
    let root = std::env::temp_dir().join(format!("cairn-git-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docker/scripts/make-fixture-home");
    assert!(Command::new("python3").arg("-I").arg(script).arg(&root).status().unwrap().success());
    let manifest = std::fs::read_to_string(root.join(".cairn-fixture")).unwrap();
    (root, serde_json::from_str(&manifest).unwrap())
}

/// Every file under `dir` with its bytes and modification time.
fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>, std::time::SystemTime)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap().flatten() {
            let meta = entry.metadata().unwrap();
            if meta.is_dir() {
                stack.push(entry.path());
            } else {
                out.push((entry.path(), std::fs::read(entry.path()).unwrap(), meta.modified().unwrap()));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[test]
fn e7_01_branch_and_changes_and_nothing_written() {
    let (root, manifest) = fixture("chip");
    let rel = "Documents/Projects/web-app";
    let expected = &manifest["git"][rel];
    let project = root.join(rel);
    let before = snapshot(&project);
    let info = git::info(&project.join("src")).expect("a repository");
    assert_eq!(info.root, project);
    assert_eq!(info.branch, expected["branch"].as_str().unwrap());
    assert_eq!(info.changes as u64, expected["changes"].as_u64().unwrap());
    assert_eq!(snapshot(&project), before, "the repository is byte-for-byte unchanged");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn e7_02_no_repository_and_a_corrupt_one() {
    let (root, _) = fixture("none");
    assert_eq!(git::info(&root.join("Documents/Invoices")), None, "not a repository");
    let broken = root.join("Documents/School");
    std::fs::create_dir_all(broken.join(".git")).unwrap();
    std::fs::write(broken.join(".git/HEAD"), "not a ref at all\n").unwrap();
    assert_eq!(git::info(&broken), None, "a corrupt .git gives no chip");
    let _ = std::fs::remove_dir_all(&root);
}
