//! L1 against the manifest (P4-T02): the Tidy plan for the fixture's Downloads.

use std::path::{Path, PathBuf};
use std::process::Command;

use cairn_core::plan::{self, SkipReason};

fn fixture(name: &str) -> (PathBuf, serde_json::Value) {
    let root = std::env::temp_dir().join(format!("cairn-plan-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docker/scripts/make-fixture-home");
    assert!(
        Command::new("python3")
            .arg("-I")
            .arg(script)
            .arg(&root)
            .output()
            .unwrap()
            .status
            .success()
    );
    let manifest = std::fs::read_to_string(root.join(".cairn-fixture")).unwrap();
    (root, serde_json::from_str(&manifest).unwrap())
}

#[test]
fn tidy_plan_equals_the_manifest() {
    let (root, manifest) = fixture("tidy");
    let downloads = root.join("Downloads");
    let plan = plan::tidy(&downloads, plan::now()).unwrap();
    let mut moves: Vec<(String, String)> = plan
        .moves
        .iter()
        .map(|m| {
            (
                m.from.strip_prefix(&root).unwrap().display().to_string(),
                m.to.strip_prefix(&root).unwrap().display().to_string(),
            )
        })
        .collect();
    moves.sort();
    let mut expected: Vec<(String, String)> = manifest["tidy"]["moves"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            (
                m["from"].as_str().unwrap().to_string(),
                m["to"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    expected.sort();
    assert_eq!(moves, expected);
    let mut left: Vec<String> = plan
        .skipped
        .iter()
        .filter(|s| s.reason == SkipReason::Downloading)
        .map(|s| s.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    left.sort();
    let mut expected_left: Vec<String> = manifest["tidy"]["left"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    expected_left.sort();
    assert_eq!(left, expected_left);
    // No folder is ever moved.
    assert!(plan.moves.iter().all(|m| !m.from.is_dir()));
    let _ = std::fs::remove_dir_all(&root);
}
