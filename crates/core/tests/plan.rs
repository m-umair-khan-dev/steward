//! L1 against the manifest (P4-T02): the Tidy plan for the fixture's Downloads.

use std::path::{Path, PathBuf};
use std::process::Command;

use steward_core::plan::{self, SkipReason};

fn fixture(name: &str) -> (PathBuf, serde_json::Value) {
    let root = std::env::temp_dir().join(format!("steward-plan-{name}-{}", std::process::id()));
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
    let manifest = std::fs::read_to_string(root.join(".steward-fixture")).unwrap();
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

#[test]
fn duplicates_match_the_planted_ones() {
    use steward_core::dupes;
    let (root, manifest) = fixture("dupes");
    let mut files = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            let meta = path.symlink_metadata().unwrap();
            if meta.is_dir() {
                stack.push(path);
            } else if meta.is_file() && path.file_name().unwrap() != ".steward-fixture" {
                files.push(path);
            }
        }
    }
    let cache = dupes::HashCache::in_memory().unwrap();
    let found: Vec<Vec<String>> = dupes::find(&files, &cache, &|| true)
        .into_iter()
        .map(|g| {
            g.iter()
                .map(|p| p.strip_prefix(&root).unwrap().display().to_string())
                .collect()
        })
        .collect();
    let mut expected: Vec<Vec<String>> = manifest["duplicates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| {
            let mut paths: Vec<String> = g
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p.as_str().unwrap().to_string())
                .collect();
            paths.sort();
            paths
        })
        .collect();
    expected.sort();
    assert_eq!(found, expected);
    // A second look reads the cache: same answer.
    let again: usize = dupes::find(&files, &cache, &|| true).len();
    assert_eq!(again, expected.len());
    let _ = std::fs::remove_dir_all(&root);
}

struct FakePackages {
    installed: Option<&'static str>,
}

impl steward_core::suggest::PackageDb for FakePackages {
    fn deb_fields(&self, deb: &Path) -> Option<(String, String)> {
        steward_core::suggest::Dpkg.deb_fields(deb)
    }
    fn installed(&self, _package: &str) -> Option<String> {
        self.installed.map(str::to_string)
    }
    fn at_least(&self, a: &str, b: &str) -> bool {
        steward_core::suggest::Dpkg.at_least(a, b)
    }
}

#[test]
fn downloads_suggestions() {
    use steward_core::suggest;
    let (root, manifest) = fixture("suggest");
    let downloads = root.join("Downloads");
    let cache = steward_core::dupes::HashCache::in_memory().unwrap();
    let now = glib::DateTime::from_iso8601(manifest["now"].as_str().unwrap(), None)
        .unwrap()
        .to_unix();
    let same = suggest::downloads(
        &downloads,
        now,
        &FakePackages {
            installed: Some("3.2.0"),
        },
        None,
        &cache,
    );
    let names = |items: &[suggest::Item]| -> Vec<String> {
        items
            .iter()
            .map(|i| i.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    };
    assert_eq!(
        names(&same.installers),
        vec!["photo-tool_3.2.0_amd64.deb".to_string()]
    );
    let older = suggest::downloads(
        &downloads,
        now,
        &FakePackages {
            installed: Some("3.1.0"),
        },
        None,
        &cache,
    );
    assert!(
        older.installers.is_empty(),
        "an older installed version: keep the installer"
    );
    let none = suggest::downloads(
        &downloads,
        now,
        &FakePackages { installed: None },
        None,
        &cache,
    );
    assert!(none.installers.is_empty());
    // Untouched for 90 days, as the manifest counts them.
    let mut untouched = names(&same.untouched);
    untouched.sort();
    let mut expected: Vec<String> = manifest["downloads"]["untouched_90_days"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    expected.sort();
    assert_eq!(untouched, expected);
    // Every duplicate suggested has an identical file elsewhere in the list.
    assert!(same.duplicates.iter().all(|d| d.why.starts_with("same as")));
    let _ = std::fs::remove_dir_all(&root);
}
