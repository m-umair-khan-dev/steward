//! L2 tests for the index (P3-T01, P3-T02, P3-T03, P3-T08) on a real fixture
//! home, and the E3-10 timing on a generated index of one million entries.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use cairn_core::index::{Index, Rules};

struct Fixture {
    root: PathBuf,
    manifest: serde_json::Value,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("cairn-index-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let script =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docker/scripts/make-fixture-home");
        let status = Command::new("python3")
            .arg("-I")
            .arg(script)
            .arg(&root)
            .output()
            .unwrap();
        assert!(status.status.success(), "fixture generator failed");
        let manifest = std::fs::read_to_string(root.join(".cairn-fixture")).unwrap();
        Self {
            root,
            manifest: serde_json::from_str(&manifest).unwrap(),
        }
    }

    fn crawl(&self) -> Index {
        let mut index = Index::open_in_memory().unwrap();
        let stats = index
            .crawl(&self.root, &Rules::for_home(&self.root), &|| true)
            .unwrap();
        assert!(stats.finished);
        index
    }

    fn folder(&self, name: &str) -> &serde_json::Value {
        &self.manifest["folders"][name]
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn names(hits: &[cairn_core::index::Hit]) -> Vec<String> {
    hits.iter().map(|h| h.name.clone()).collect()
}

#[test]
fn e3_01_every_file_is_indexed() {
    let fixture = Fixture::new("count");
    let index = fixture.crawl();
    let (_, files) = index.counts();
    assert_eq!(files, fixture.manifest["totals"]["files"].as_u64().unwrap());
    assert!(index.is_complete());
}

#[test]
fn e3_02_folder_sizes_and_counts_match_the_manifest() {
    let fixture = Fixture::new("stats");
    let index = fixture.crawl();
    for name in [
        "Documents/Invoices",
        "Documents/Manuals",
        "Documents/Clients",
        "Documents/Projects",
        "Downloads",
        "Downloads/Receipts",
        "Pictures",
    ] {
        let expected = fixture.folder(name);
        let stats = index.folder_stats(&fixture.root.join(name)).unwrap();
        assert_eq!(
            stats.items,
            expected["items"].as_u64().unwrap(),
            "{name} items"
        );
        assert_eq!(
            stats.bytes,
            expected["tree_bytes"].as_u64().unwrap(),
            "{name} bytes"
        );
        assert_eq!(
            stats.files,
            expected["tree_files"].as_u64().unwrap(),
            "{name} files"
        );
    }
}

#[test]
fn search_ranks_prefix_first_and_hides_dotfiles() {
    let fixture = Fixture::new("search");
    let index = fixture.crawl();
    let hits = index.search("invo", 50);
    assert!(!hits.is_empty());
    assert!(hits.iter().all(|h| h.name.to_lowercase().contains("invo")));
    // Every name that starts with the query comes before any that only contains it.
    let first_contains = hits
        .iter()
        .position(|h| !h.name.to_lowercase().starts_with("invo"));
    if let Some(at) = first_contains {
        assert!(
            hits[at..]
                .iter()
                .all(|h| !h.name.to_lowercase().starts_with("invo"))
        );
    }
    assert!(names(&hits).iter().any(|n| n == "Invoices"));
    assert!(
        index
            .search("npm", 50)
            .iter()
            .all(|h| !h.path.to_string_lossy().contains("/."))
    );
    // Short queries still work (by prefix).
    assert!(
        index
            .search("In", 50)
            .iter()
            .all(|h| h.name.to_lowercase().starts_with("in"))
    );
}

#[test]
fn e3_09_folders_by_name() {
    let fixture = Fixture::new("named");
    let index = fixture.crawl();
    let hits = index.folders_named("Invoices", 10);
    let paths: Vec<PathBuf> = hits
        .iter()
        .filter(|h| h.name == "Invoices")
        .map(|h| h.path.clone())
        .collect();
    assert!(paths.contains(&fixture.root.join("Documents/Invoices")));
    assert!(paths.contains(&fixture.root.join("Documents/Clients/Northwind/Invoices")));
    assert!(hits.iter().all(|h| h.is_dir));
}

#[test]
fn refresh_keeps_the_roll_up_right_to_the_top() {
    let fixture = Fixture::new("refresh");
    let mut index = fixture.crawl();
    let rules = Rules::for_home(&fixture.root);
    let invoices = fixture.root.join("Documents/Invoices");
    let before_dir = index.folder_stats(&invoices).unwrap();
    let before_home = index.folder_stats(&fixture.root).unwrap();

    std::fs::write(invoices.join("Invoice-1001.pdf"), vec![7u8; 1000]).unwrap();
    index.refresh_dir(&invoices, &rules).unwrap();
    let after = index.folder_stats(&invoices).unwrap();
    assert_eq!(
        (after.items, after.bytes, after.files),
        (
            before_dir.items + 1,
            before_dir.bytes + 1000,
            before_dir.files + 1
        )
    );
    assert_eq!(
        index.folder_stats(&fixture.root).unwrap().bytes,
        before_home.bytes + 1000
    );
    assert_eq!(
        names(&index.search("Invoice-1001", 5)),
        vec!["Invoice-1001.pdf".to_string()]
    );

    // A new folder with files inside is crawled completely.
    let new = fixture.root.join("Documents/Taxes 2027");
    std::fs::create_dir_all(new.join("Q1")).unwrap();
    std::fs::write(new.join("Q1/a.txt"), "12345").unwrap();
    index
        .refresh_dir(&fixture.root.join("Documents"), &rules)
        .unwrap();
    assert_eq!(index.folder_stats(&new).unwrap().bytes, 5);
    assert_eq!(index.folder_stats(&new).unwrap().items, 1);

    // A rename: the old name is gone, the new one is found.
    std::fs::rename(
        invoices.join("Invoice-1001.pdf"),
        invoices.join("Renamed-1001.pdf"),
    )
    .unwrap();
    index.refresh_dir(&invoices, &rules).unwrap();
    assert!(index.search("Invoice-1001", 5).is_empty());
    assert_eq!(index.search("Renamed-1001", 5).len(), 1);

    // Removing a folder removes everything under it and fixes the totals.
    std::fs::remove_dir_all(&new).unwrap();
    index.refresh_dir(&new, &rules).unwrap();
    assert!(index.folder_stats(&new).is_none());
    assert!(
        index
            .search("a.txt", 5)
            .iter()
            .all(|h| !h.path.starts_with(&new))
    );
    std::fs::remove_file(invoices.join("Renamed-1001.pdf")).unwrap();
    index.refresh_dir(&invoices, &rules).unwrap();
    assert_eq!(index.folder_stats(&fixture.root).unwrap(), before_home);
}

#[test]
fn a_new_crawl_forgets_what_is_gone_and_skips_excluded_folders() {
    let fixture = Fixture::new("recrawl");
    let mut index = fixture.crawl();
    let (entries, _) = index.counts();
    std::fs::remove_dir_all(fixture.root.join("Documents/Manuals")).unwrap();
    std::fs::create_dir_all(fixture.root.join(".cache/cairn")).unwrap();
    std::fs::write(fixture.root.join(".cache/cairn/index.db"), "x").unwrap();
    index
        .crawl(&fixture.root, &Rules::for_home(&fixture.root), &|| true)
        .unwrap();
    assert!(
        index
            .folder_stats(&fixture.root.join("Documents/Manuals"))
            .is_none()
    );
    assert!(index.counts().0 < entries);
    assert!(index.search("index.db", 5).is_empty());
}

#[test]
fn an_interrupted_crawl_keeps_what_it_had() {
    let fixture = Fixture::new("stop");
    let mut index = fixture.crawl();
    let (entries, _) = index.counts();
    let stats = index
        .crawl(&fixture.root, &Rules::for_home(&fixture.root), &|| false)
        .unwrap();
    assert!(!stats.finished);
    assert_eq!(index.counts().0, entries, "nothing was removed");
}

#[test]
fn outline_is_two_levels_folders_first() {
    let fixture = Fixture::new("outline");
    let index = fixture.crawl();
    let lines = index.outline(&fixture.root.join("Documents"), 100);
    assert!(lines.iter().any(|l| l.depth == 1));
    assert!(lines.iter().all(|l| l.depth <= 1));
    let top: Vec<_> = lines.iter().filter(|l| l.depth == 0).collect();
    let first_file = top.iter().position(|l| !l.is_dir).unwrap_or(top.len());
    assert!(top[first_file..].iter().all(|l| !l.is_dir), "folders first");
}

#[test]
fn e3_10_a_million_entries_answer_within_50_ms() {
    let dir = std::env::temp_dir().join(format!("cairn-e3-10-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("index.db");
    let mut index = Index::open(&path).unwrap();
    let started = Instant::now();
    index.fill_synthetic(1_000_000).unwrap();
    let filled = started.elapsed();

    let queries = [
        "invo",
        "report",
        "IMG_",
        "budget",
        "2026",
        "draft",
        "photo",
        "zz9",
        "readme",
        "png",
        "tax",
        "final",
        "screenshot",
        "notes",
        "a",
        "logo",
        "contract",
        "q3",
        "holiday",
        "deck",
    ];
    let mut times = Vec::new();
    for round in 0..5 {
        for query in queries {
            let started = Instant::now();
            let hits = index.search(query, 50);
            times.push(started.elapsed().as_secs_f64() * 1000.0);
            if round == 0 && query == "invo" {
                assert!(!hits.is_empty());
            }
        }
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p95 = times[(times.len() as f64 * 0.95) as usize - 1];
    let report = format!(
        "{{\"entries\": 1000000, \"fill_seconds\": {:.1}, \"queries\": {}, \"p50_ms\": {:.2}, \"p95_ms\": {:.2}, \"max_ms\": {:.2}}}\n",
        filled.as_secs_f64(),
        times.len(),
        times[times.len() / 2],
        p95,
        times[times.len() - 1]
    );
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/perf");
    let _ = std::fs::create_dir_all(&out);
    let _ = std::fs::write(out.join("E3-10.json"), &report);
    println!("E3-10: {report}");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        p95 <= 50.0,
        "95th percentile {p95:.1} ms is over 50 ms ({report})"
    );
}
