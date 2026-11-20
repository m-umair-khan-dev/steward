//! L2 per suggester (P5-T01, P5-T03): every Smart Clean card against the
//! fixture home's planted data and its manifest.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use cairn_core::clean::{self, Card, Context, Reason};
use cairn_core::tools::{Ran, Runner};

fn fixture(name: &str) -> (PathBuf, serde_json::Value) {
    let root = std::env::temp_dir().join(format!("cairn-clean-{name}-{}", std::process::id()));
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

/// The system as the manifest describes it: photo-tool 3.2.0 installed,
/// org.example.Kept an installed Flatpak, no Snaps.
struct FakePackages;

impl cairn_core::suggest::PackageDb for FakePackages {
    fn deb_fields(&self, deb: &Path) -> Option<(String, String)> {
        cairn_core::suggest::Dpkg.deb_fields(deb)
    }
    fn installed(&self, package: &str) -> Option<String> {
        (package == "photo-tool").then(|| "3.2.0".to_string())
    }
    fn at_least(&self, a: &str, b: &str) -> bool {
        cairn_core::suggest::Dpkg.at_least(a, b)
    }
}

#[derive(Default)]
struct FakeTools {
    calls: Mutex<Vec<String>>,
}

impl Runner for FakeTools {
    fn run(&self, program: &str, args: &[&str]) -> Ran {
        self.calls
            .lock()
            .unwrap()
            .push(format!("{program} {}", args.join(" ")));
        match program {
            "flatpak" => Ran::Output("org.example.Kept\n".into()),
            "snap" => Ran::Output("Name  Version  Rev  Tracking  Publisher  Notes\n".into()),
            _ => Ran::Missing,
        }
    }
}

fn context(root: &Path, manifest: &serde_json::Value, tools: Arc<FakeTools>) -> Context {
    let now = glib::DateTime::from_iso8601(manifest["now"].as_str().unwrap(), None)
        .unwrap()
        .to_unix();
    Context {
        home: root.to_path_buf(),
        downloads: root.join("Downloads"),
        desktop: root.join("Desktop"),
        videos: root.join("Videos"),
        now,
        packages: Arc::new(FakePackages),
        runner: tools,
        hash_cache: None,
    }
}

type Row = (String, u64, bool, bool);

fn rows(card: &Card, root: &Path) -> Vec<Row> {
    let mut out: Vec<Row> = card
        .items
        .iter()
        .map(|i| {
            (
                i.path.strip_prefix(root).unwrap().display().to_string(),
                i.bytes,
                i.ticked,
                i.keep,
            )
        })
        .collect();
    out.sort();
    out
}

fn expected(manifest: &serde_json::Value, card: &str) -> Vec<Row> {
    let mut out: Vec<Row> = manifest["clean"]["cards"][card]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            (
                i["path"].as_str().unwrap().to_string(),
                i["bytes"].as_u64().unwrap(),
                i["ticked"].as_bool().unwrap(),
                i["keep"].as_bool().unwrap_or(false),
            )
        })
        .collect();
    out.sort();
    out
}

#[test]
fn every_card_matches_the_manifest() {
    let (root, manifest) = fixture("cards");
    let cards = clean::scan_all(&context(&root, &manifest, Arc::default()));
    let ids: Vec<&str> = cards.iter().map(|c| c.id).collect();
    assert_eq!(
        ids,
        [
            "dev_caches",
            "duplicates",
            "installers",
            "stale_builds",
            "leftovers",
            "big_files",
            "screencasts",
            "old_downloads"
        ]
    );
    for card in &cards {
        assert_eq!(
            rows(card, &root),
            expected(&manifest, card.id),
            "{}",
            card.id
        );
        assert_eq!(
            card.total(),
            manifest["clean"]["totals"][card.id].as_u64().unwrap(),
            "{} total",
            card.id
        );
    }
    let ticked: u64 = cards
        .iter()
        .flat_map(|c| &c.items)
        .filter(|i| i.ticked)
        .map(|i| i.bytes)
        .sum();
    assert_eq!(ticked, manifest["clean"]["ticked_bytes"].as_u64().unwrap());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn folders_called_cache_off_the_allow_list_are_never_listed() {
    let (root, manifest) = fixture("not-listed");
    let cards = clean::scan_all(&context(&root, &manifest, Arc::default()));
    for path in manifest["clean"]["not_listed"].as_array().unwrap() {
        let path = root.join(path.as_str().unwrap());
        for card in &cards {
            assert!(
                card.items.iter().all(|i| !i.path.starts_with(&path)),
                "{} lists {}",
                card.id,
                path.display()
            );
        }
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn reasons_say_why() {
    let (root, manifest) = fixture("reasons");
    let cards = clean::scan_all(&context(&root, &manifest, Arc::default()));
    let card = |id: &str| cards.iter().find(|c| c.id == id).unwrap();
    let invoice = card("duplicates")
        .items
        .iter()
        .find(|i| i.path.ends_with("Downloads/Invoice-0923.pdf"))
        .unwrap();
    assert_eq!(
        invoice.reason,
        Reason::SameAs {
            kept: root.join("Documents/Invoices/Invoice-0923.pdf")
        }
    );
    assert!(matches!(
        &card("installers").items[0].reason,
        Reason::Installed { package, version } if package == "photo-tool" && version == "3.2.0"
    ));
    assert!(matches!(
        &card("leftovers").items[0].reason,
        Reason::Leftover { app, tool: "flatpak", days } if app == "org.example.Removed" && *days >= 90
    ));
    assert!(matches!(
        &card("stale_builds").items[0].reason,
        Reason::StaleBuild { days, .. } if *days >= 182
    ));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_parallel_scan_reports_every_card_once() {
    let (root, manifest) = fixture("parallel");
    let ctx = Arc::new(context(&root, &manifest, Arc::default()));
    let (send, receive) = std::sync::mpsc::channel();
    let send = Mutex::new(send);
    clean::scan(
        ctx,
        Arc::new(|| true),
        Arc::new(move |card: Card| send.lock().unwrap().send(card).unwrap()),
    );
    let mut cards: Vec<Card> = (0..clean::registry().len())
        .map(|_| {
            receive
                .recv_timeout(std::time::Duration::from_secs(60))
                .unwrap()
        })
        .collect();
    let order: Vec<&str> = clean::registry().iter().map(|s| s.id()).collect();
    cards.sort_by_key(|c| order.iter().position(|id| *id == c.id));
    clean::exclusive(&mut cards);
    for card in &cards {
        assert_eq!(
            rows(card, &root),
            expected(&manifest, card.id),
            "{}",
            card.id
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn only_read_only_queries_are_run() {
    let (root, manifest) = fixture("queries");
    let tools = Arc::new(FakeTools::default());
    let _ = clean::scan_all(&context(&root, &manifest, tools.clone()));
    let _ = clean::admin::rows(&*tools);
    let calls = tools.calls.lock().unwrap().clone();
    for call in &calls {
        assert!(
            [
                "flatpak list",
                "snap list",
                "journalctl --disk-usage",
                "dpkg-query -W",
                "docker system df"
            ]
            .iter()
            .any(|ok| call.starts_with(ok)),
            "unexpected call: {call}"
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}
