//! The eight Smart Clean suggesters (DESIGN.md §4.3, P5-T03). Each one only
//! reads; what it returns is a suggestion with a reason.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use super::{
    Context, DAY, Group, Item, Reason, Suggester, tree_bytes, tree_latest, visible_files,
};
use crate::dupes::{self, HashCache};

/// Files at least this big are "big files".
pub const BIG_FILE: u64 = 256 * 1024 * 1024;
/// At most this many big files are listed.
const BIG_FILES_SHOWN: usize = 50;
/// A project untouched this long has stale build output (6 months).
pub const STALE_DAYS: i64 = 182;
/// App data untouched this long is a leftover (3 months).
pub const LEFTOVER_DAYS: i64 = 90;
/// Screen recordings and downloads older than this are suggested.
pub const OLD_DAYS: i64 = 90;
/// Installers must be at least this old to be ticked.
const INSTALLER_TICK_DAYS: i64 = 7;

const PARTIAL_SUFFIXES: [&str; 4] = [".crdownload", ".part", ".partial", ".download"];

fn is_partial(path: &Path) -> bool {
    let name = path.to_string_lossy().to_lowercase();
    PARTIAL_SUFFIXES.iter().any(|s| name.ends_with(s))
}

fn age_days(ctx: &Context, stamp: i64) -> i64 {
    (ctx.now - stamp) / DAY
}

fn item(path: PathBuf, bytes: u64, reason: Reason, ticked: bool) -> Item {
    Item {
        path,
        bytes,
        reason,
        ticked,
        keep: false,
    }
}

// --- Safe to clear ----------------------------------------------------------------

/// Download and build caches on the allow-list, nothing else.
pub struct DevCaches;

impl Suggester for DevCaches {
    fn id(&self) -> &'static str {
        "dev_caches"
    }
    fn group(&self) -> Group {
        Group::Safe
    }
    fn find(&self, ctx: &Context, _: &dyn Fn() -> bool) -> Vec<Item> {
        super::dev_caches()
            .into_iter()
            .filter_map(|cache| {
                let path = ctx.home.join(&cache.path);
                // A link in the cache's place is not the cache: never follow it.
                let meta = path.symlink_metadata().ok()?;
                if !meta.is_dir() {
                    return None;
                }
                let bytes = tree_bytes(&path);
                (bytes > 0).then(|| item(path, bytes, Reason::Cache { cache }, true))
            })
            .collect()
    }
}

/// True for names like "report (2).pdf", "photo copy.jpg", "Copy of notes.txt".
pub fn copy_like(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let stem = match name.rfind('.') {
        Some(dot) if dot > 0 => &name[..dot],
        _ => name.as_str(),
    };
    if name.starts_with("copy of ") {
        return true;
    }
    let paren = stem
        .strip_suffix(')')
        .and_then(|s| s.rsplit_once(" ("))
        .is_some_and(|(_, n)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    let copy = stem.ends_with(" copy")
        || stem
            .rsplit_once(" copy ")
            .is_some_and(|(_, n)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    paren || copy
}

/// The same contents in more than one place. One copy is always kept,
/// preferring a filed one (not in Downloads or on the Desktop, not named
/// like a copy), then the oldest. Only spare copies that are loose in
/// Downloads or on the Desktop, or named like a copy, are ticked.
pub struct Duplicates;

impl Suggester for Duplicates {
    fn id(&self) -> &'static str {
        "duplicates"
    }
    fn group(&self) -> Group {
        Group::Safe
    }
    fn find(&self, ctx: &Context, keep_going: &dyn Fn() -> bool) -> Vec<Item> {
        let files: Vec<PathBuf> = visible_files(&ctx.home, keep_going)
            .into_iter()
            .map(|(p, _)| p)
            .collect();
        let cache = ctx
            .hash_cache
            .as_deref()
            .and_then(|p| HashCache::open(p).ok())
            .or_else(|| HashCache::in_memory().ok());
        let Some(cache) = cache else {
            return Vec::new();
        };
        let loose = |p: &Path| p.starts_with(&ctx.downloads) || p.starts_with(&ctx.desktop);
        let mut out = Vec::new();
        for group in dupes::find(&files, &cache, keep_going) {
            let mtime = |p: &Path| p.symlink_metadata().map(|m| m.mtime()).unwrap_or(0);
            let Some(keep) = group
                .iter()
                .min_by_key(|p| {
                    (
                        loose(p),
                        copy_like(p),
                        mtime(p),
                        p.as_os_str().len(),
                        (*p).clone(),
                    )
                })
                .cloned()
            else {
                continue;
            };
            // The kept copy first, then its spare copies.
            let mut group = group;
            group.sort_by_key(|p| *p != keep);
            for path in group {
                let bytes = path.symlink_metadata().map(|m| m.len()).unwrap_or(0);
                if path == keep {
                    out.push(Item {
                        path,
                        bytes,
                        reason: Reason::KeptCopy,
                        ticked: false,
                        keep: true,
                    });
                } else {
                    let ticked = loose(&path) || copy_like(&path);
                    out.push(item(
                        path,
                        bytes,
                        Reason::SameAs { kept: keep.clone() },
                        ticked,
                    ));
                }
            }
        }
        out
    }
}

/// `.deb` files for packages that are installed. Ticked only when the file
/// is in Downloads or on the Desktop, is not newer than what is installed,
/// and is more than a week old.
pub struct Installers;

impl Suggester for Installers {
    fn id(&self) -> &'static str {
        "installers"
    }
    fn group(&self) -> Group {
        Group::Safe
    }
    fn find(&self, ctx: &Context, keep_going: &dyn Fn() -> bool) -> Vec<Item> {
        let mut out = Vec::new();
        for (path, meta) in visible_files(&ctx.home, keep_going) {
            if !path.to_string_lossy().to_lowercase().ends_with(".deb") {
                continue;
            }
            let Some((package, version)) = ctx.packages.deb_fields(&path) else {
                continue;
            };
            let Some(installed) = ctx.packages.installed(&package) else {
                continue;
            };
            let not_newer = ctx.packages.at_least(&installed, &version);
            let loose = path.starts_with(&ctx.downloads) || path.starts_with(&ctx.desktop);
            let ticked =
                loose && not_newer && age_days(ctx, meta.mtime()) > INSTALLER_TICK_DAYS;
            out.push(item(
                path,
                meta.len(),
                Reason::Installed {
                    package,
                    version: installed,
                },
                ticked,
            ));
        }
        out
    }
}

// --- Your call ---------------------------------------------------------------------

/// A project marker and the build output it makes.
const BUILD_OUTPUT: [(&str, &str); 9] = [
    ("package.json", "node_modules"),
    ("Cargo.toml", "target"),
    ("pyproject.toml", ".venv"),
    ("setup.py", ".venv"),
    ("requirements.txt", ".venv"),
    ("build.gradle", "build"),
    ("build.gradle.kts", "build"),
    ("CMakeLists.txt", "build"),
    ("meson.build", "build"),
];

/// `node_modules`, `target`, `.venv` and `build` in projects not touched
/// for six months. The project's age ignores the build output itself.
pub struct StaleBuilds;

impl Suggester for StaleBuilds {
    fn id(&self) -> &'static str {
        "stale_builds"
    }
    fn group(&self) -> Group {
        Group::YourCall
    }
    fn find(&self, ctx: &Context, keep_going: &dyn Fn() -> bool) -> Vec<Item> {
        let outputs: Vec<&str> = BUILD_OUTPUT.iter().map(|(_, o)| *o).collect();
        let mut out = Vec::new();
        let mut stack = vec![ctx.home.clone()];
        while let Some(dir) = stack.pop() {
            if !keep_going() {
                break;
            }
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut children: Vec<(String, std::fs::Metadata)> = entries
                .flatten()
                .filter_map(|e| {
                    let meta = e.path().symlink_metadata().ok()?;
                    Some((e.file_name().to_string_lossy().into_owned(), meta))
                })
                .collect();
            children.sort_by(|a, b| a.0.cmp(&b.0));
            let has = |name: &str| children.iter().any(|(n, m)| n == name && m.is_file());
            let mut found: Vec<&str> = BUILD_OUTPUT
                .iter()
                .filter(|(marker, output)| {
                    has(marker) && children.iter().any(|(n, m)| n == output && m.is_dir())
                })
                .map(|(_, output)| *output)
                .collect();
            found.dedup();
            if !found.is_empty() {
                let latest = tree_latest(&dir, false, &outputs);
                let days = age_days(ctx, latest);
                if days >= STALE_DAYS {
                    for output in &found {
                        let path = dir.join(output);
                        out.push(item(
                            path.clone(),
                            tree_bytes(&path),
                            Reason::StaleBuild {
                                project: dir.clone(),
                                days,
                            },
                            false,
                        ));
                    }
                }
            }
            for (name, meta) in children {
                if meta.is_dir() && !name.starts_with('.') && !found.contains(&name.as_str()) {
                    stack.push(dir.join(name));
                }
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }
}

/// Lists installed apps of a tool: Some(ids) (empty when the tool itself is
/// not installed), or None when the tool could not be asked.
fn installed_ids(ctx: &Context, program: &str, args: &[&str], column: usize) -> Option<Vec<String>> {
    match ctx.runner.run(program, args) {
        crate::tools::Ran::Missing => Some(Vec::new()),
        crate::tools::Ran::Failed(_) => None,
        crate::tools::Ran::Output(text) => Some(
            text.lines()
                .filter_map(|l| l.split_whitespace().nth(column).map(str::to_string))
                .collect(),
        ),
    }
}

/// Data folders of Flatpaks and Snaps that are no longer installed
/// (`~/.var/app/<id>`, `~/snap/<name>`), untouched for three months.
pub struct Leftovers;

impl Suggester for Leftovers {
    fn id(&self) -> &'static str {
        "leftovers"
    }
    fn group(&self) -> Group {
        Group::YourCall
    }
    fn find(&self, ctx: &Context, _: &dyn Fn() -> bool) -> Vec<Item> {
        let opened = crate::suggest::atime_trusted(&ctx.home);
        let mut out = Vec::new();
        let places: [(&str, PathBuf, Option<Vec<String>>); 2] = [
            (
                "flatpak",
                ctx.home.join(".var/app"),
                installed_ids(ctx, "flatpak", &["list", "--columns=application"], 0),
            ),
            (
                "snap",
                ctx.home.join("snap"),
                // `snap list` starts with a header row ("Name Version …").
                installed_ids(ctx, "snap", &["list"], 0).map(|mut ids| {
                    if ids.first().is_some_and(|h| h == "Name") {
                        ids.remove(0);
                    }
                    ids
                }),
            ),
        ];
        for (tool, folder, installed) in places {
            let Some(installed) = installed else { continue };
            let Ok(entries) = std::fs::read_dir(&folder) else {
                continue;
            };
            let mut apps: Vec<PathBuf> = entries
                .flatten()
                .filter(|e| e.path().symlink_metadata().is_ok_and(|m| m.is_dir()))
                .map(|e| e.path())
                .collect();
            apps.sort();
            for path in apps {
                let app = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if installed.contains(&app) {
                    continue;
                }
                let days = age_days(ctx, tree_latest(&path, opened, &[]));
                if days >= LEFTOVER_DAYS {
                    out.push(item(
                        path.clone(),
                        tree_bytes(&path),
                        Reason::Leftover { app, tool, days },
                        false,
                    ));
                }
            }
        }
        out
    }
}

/// The largest files in the home folder, disk images included.
pub struct BigFiles;

impl Suggester for BigFiles {
    fn id(&self) -> &'static str {
        "big_files"
    }
    fn group(&self) -> Group {
        Group::YourCall
    }
    fn find(&self, ctx: &Context, keep_going: &dyn Fn() -> bool) -> Vec<Item> {
        let mut big: Vec<(PathBuf, u64)> = visible_files(&ctx.home, keep_going)
            .into_iter()
            .filter(|(p, m)| m.len() >= BIG_FILE && !is_partial(p))
            .map(|(p, m)| (p, m.len()))
            .collect();
        big.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        big.truncate(BIG_FILES_SHOWN);
        big.into_iter()
            .map(|(p, bytes)| item(p, bytes, Reason::Big, false))
            .collect()
    }
}

/// Screen recordings (`~/Videos/Screencasts`) older than 90 days.
pub struct Screencasts;

impl Suggester for Screencasts {
    fn id(&self) -> &'static str {
        "screencasts"
    }
    fn group(&self) -> Group {
        Group::YourCall
    }
    fn find(&self, ctx: &Context, keep_going: &dyn Fn() -> bool) -> Vec<Item> {
        visible_files(&ctx.videos.join("Screencasts"), keep_going)
            .into_iter()
            .filter_map(|(path, meta)| {
                let days = age_days(ctx, meta.mtime());
                (days >= OLD_DAYS)
                    .then(|| item(path, meta.len(), Reason::OldRecording { days }, false))
            })
            .collect()
    }
}

/// Loose files in Downloads not opened, changed or added for 90 days.
pub struct OldDownloads;

impl Suggester for OldDownloads {
    fn id(&self) -> &'static str {
        "old_downloads"
    }
    fn group(&self) -> Group {
        Group::YourCall
    }
    fn find(&self, ctx: &Context, _: &dyn Fn() -> bool) -> Vec<Item> {
        let opened = crate::suggest::atime_trusted(&ctx.downloads);
        let Ok(entries) = std::fs::read_dir(&ctx.downloads) else {
            return Vec::new();
        };
        let mut out: Vec<Item> = entries
            .flatten()
            .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .filter_map(|e| {
                let path = e.path();
                let meta = path.symlink_metadata().ok()?;
                if !meta.is_file() || is_partial(&path) {
                    return None;
                }
                let last = if opened {
                    meta.mtime().max(meta.atime())
                } else {
                    meta.mtime()
                };
                let days = age_days(ctx, last);
                (days >= OLD_DAYS)
                    .then(|| item(path, meta.len(), Reason::OldDownload { days, opened }, false))
            })
            .collect();
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::super::visible;
    use super::*;

    #[test]
    fn copy_names_are_recognised() {
        for name in ["a (1).pdf", "Invoice-0923 (12).pdf", "photo copy.jpg", "photo copy 2.jpg",
                     "Copy of notes.txt", "README (3)"] {
            assert!(copy_like(Path::new(name)), "{name}");
        }
        for name in ["logo-final.png", "copy.txt", "(1).pdf", "a (x).pdf", "photocopy.jpg"] {
            assert!(!copy_like(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn visible_means_no_hidden_part_below_home() {
        let home = Path::new("/h");
        assert!(visible(Path::new("/h/Documents/a.txt"), home));
        assert!(!visible(Path::new("/h/.cache/x"), home));
        assert!(visible(Path::new("/elsewhere/.x"), home));
    }
}
