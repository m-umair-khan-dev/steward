//! Smart Clean (DESIGN.md §4.3, Phase 5): suggestions for getting space
//! back, each with a reason. The rule shared by everything here: read first,
//! change only what the user ticked, and only through the Trash (the ops
//! engine does that part; nothing in this module changes a file).
//!
//! Each card is a [`Suggester`]. [`scan`] runs them all at once and reports
//! each card as it finishes; [`exclusive`] then makes sure no path is counted
//! by two cards (the first card in [`registry`] order keeps it).

pub mod admin;
mod caches;
pub mod disk;
mod finders;

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use caches::{DevCache, dev_caches};

use crate::suggest::PackageDb;
use crate::tools::Runner;

/// Where a card sits on the overview page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// Pre-ticked items are clearly spare.
    Safe,
    /// Never pre-ticked.
    YourCall,
}

impl Group {
    pub fn id(self) -> &'static str {
        match self {
            Group::Safe => "safe",
            Group::YourCall => "your-call",
        }
    }
}

/// Why an item is suggested. The app turns this into words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// An allow-listed cache; `cache` is its entry in the allow-list.
    Cache { cache: DevCache },
    /// The copy of a duplicate set that is kept (never ticked).
    KeptCopy,
    /// A spare copy; `kept` is the copy that stays.
    SameAs { kept: PathBuf },
    /// A `.deb` for a package that is installed.
    Installed { package: String, version: String },
    /// Build output of a project not touched for `days`.
    StaleBuild { project: PathBuf, days: i64 },
    /// Data of an app that is no longer installed ("flatpak" or "snap").
    Leftover { app: String, tool: &'static str, days: i64 },
    /// One of the largest files in the home folder.
    Big,
    /// A screen recording older than `days`.
    OldRecording { days: i64 },
    /// A download not opened (or, with `opened` false, not changed) for `days`.
    OldDownload { days: i64, opened: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub path: PathBuf,
    /// Apparent size: bytes of every regular file inside (links are not
    /// followed).
    pub bytes: u64,
    pub reason: Reason,
    /// Ticked when the page opens.
    pub ticked: bool,
    /// Shown, but cannot be ticked (the copy of a duplicate set that stays).
    pub keep: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub id: &'static str,
    pub group: Group,
    pub items: Vec<Item>,
}

impl Card {
    /// Bytes that could go (every item but the kept copies).
    pub fn total(&self) -> u64 {
        self.items.iter().filter(|i| !i.keep).map(|i| i.bytes).sum()
    }
}

/// What a scan needs to know about the machine.
pub struct Context {
    pub home: PathBuf,
    pub downloads: PathBuf,
    pub desktop: PathBuf,
    pub videos: PathBuf,
    /// Seconds since the epoch ([`crate::plan::now`]).
    pub now: i64,
    pub packages: Arc<dyn PackageDb>,
    pub runner: Arc<dyn Runner>,
    /// The duplicate finder's hash cache (None: in memory only).
    pub hash_cache: Option<PathBuf>,
}

impl Context {
    /// The usual places, from the XDG user folders.
    pub fn for_home(
        home: PathBuf,
        downloads: PathBuf,
        packages: Arc<dyn PackageDb>,
        runner: Arc<dyn Runner>,
    ) -> Self {
        let special = |dir, name: &str| glib::user_special_dir(dir).unwrap_or(home.join(name));
        Self {
            desktop: special(glib::UserDirectory::Desktop, "Desktop"),
            videos: special(glib::UserDirectory::Videos, "Videos"),
            downloads,
            now: crate::plan::now(),
            packages,
            runner,
            hash_cache: Some(crate::dupes::default_cache_path()),
            home,
        }
    }
}

pub trait Suggester: Send + Sync {
    fn id(&self) -> &'static str;
    fn group(&self) -> Group;
    /// The card's items. `keep_going` is asked now and then; false stops early.
    fn find(&self, ctx: &Context, keep_going: &dyn Fn() -> bool) -> Vec<Item>;
}

/// Every card, in the order the overview shows them. The "Unused apps" card
/// is not here: Linux keeps no trustworthy last-launched date (ADR 0011).
pub fn registry() -> Vec<Arc<dyn Suggester>> {
    vec![
        Arc::new(finders::DevCaches),
        Arc::new(finders::Duplicates),
        Arc::new(finders::Installers),
        Arc::new(finders::StaleBuilds),
        Arc::new(finders::Leftovers),
        Arc::new(finders::BigFiles),
        Arc::new(finders::Screencasts),
        Arc::new(finders::OldDownloads),
    ]
}

/// Runs every suggester at once, each on its own thread, and calls `report`
/// with each card as it finishes (in whatever order they finish).
pub fn scan(
    ctx: Arc<Context>,
    keep_going: Arc<dyn Fn() -> bool + Send + Sync>,
    report: Arc<dyn Fn(Card) + Send + Sync>,
) {
    for suggester in registry() {
        let (ctx, keep_going, report) = (ctx.clone(), keep_going.clone(), report.clone());
        std::thread::spawn(move || {
            let items = suggester.find(&ctx, &*keep_going);
            if keep_going() {
                report(Card {
                    id: suggester.id(),
                    group: suggester.group(),
                    items,
                });
            }
        });
    }
}

/// Runs every suggester on this thread, in order (tests and the CLI).
pub fn scan_all(ctx: &Context) -> Vec<Card> {
    let mut cards: Vec<Card> = registry()
        .iter()
        .map(|s| Card {
            id: s.id(),
            group: s.group(),
            items: s.find(ctx, &|| true),
        })
        .collect();
    exclusive(&mut cards);
    cards
}

/// Gives each path to the first card (in `cards` order) that has it, or that
/// has a folder holding it. `cards` must be in [`registry`] order.
pub fn exclusive(cards: &mut [Card]) {
    let mut claimed: Vec<PathBuf> = Vec::new();
    for card in cards.iter_mut() {
        card.items
            .retain(|item| !claimed.iter().any(|c| item.path.starts_with(c)));
        claimed.extend(card.items.iter().map(|i| i.path.clone()));
    }
}

// --- helpers shared by the suggesters ------------------------------------------------

/// True if no part of `path` below `home` starts with a dot.
#[cfg(test)]
pub(crate) fn visible(path: &Path, home: &Path) -> bool {
    path.strip_prefix(home).map_or(true, |rest| {
        !rest
            .components()
            .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
    })
}

/// Bytes of every regular file at or under `path` (links not followed).
pub fn tree_bytes(path: &Path) -> u64 {
    let Ok(meta) = path.symlink_metadata() else {
        return 0;
    };
    if meta.is_file() {
        return meta.len();
    }
    if !meta.is_dir() {
        return 0;
    }
    let mut total = 0;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.path().symlink_metadata() else {
                continue;
            };
            if meta.is_dir() {
                stack.push(entry.path());
            } else if meta.is_file() {
                total += meta.len();
            }
        }
    }
    total
}

/// The latest time anything at or under `path` was changed (and opened,
/// when `opened` is true), skipping children of folders named in `skip`.
pub(crate) fn tree_latest(path: &Path, opened: bool, skip: &[&str]) -> i64 {
    let stamp = |meta: &std::fs::Metadata| {
        if opened {
            meta.mtime().max(meta.atime())
        } else {
            meta.mtime()
        }
    };
    let Ok(meta) = path.symlink_metadata() else {
        return 0;
    };
    if !meta.is_dir() {
        return stamp(&meta);
    }
    let mut latest = 0;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.path().symlink_metadata() else {
                continue;
            };
            if meta.is_dir() {
                if !skip.iter().any(|s| entry.file_name() == **s) {
                    stack.push(entry.path());
                }
            } else {
                latest = latest.max(stamp(&meta));
            }
        }
    }
    latest
}

/// Visible regular files under `root` (hidden folders and links skipped),
/// with their metadata. `keep_going` is asked between folders.
pub(crate) fn visible_files(
    root: &Path,
    keep_going: &dyn Fn() -> bool,
) -> Vec<(PathBuf, std::fs::Metadata)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if !keep_going() {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let Ok(meta) = entry.path().symlink_metadata() else {
                continue;
            };
            if meta.is_dir() {
                stack.push(entry.path());
            } else if meta.is_file() {
                out.push((entry.path(), meta));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

pub(crate) const DAY: i64 = 86_400;
