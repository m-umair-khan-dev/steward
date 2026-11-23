//! Suggestions for the Downloads side panel (DESIGN.md §4.2, P4-T05):
//! installers for apps already installed, likely duplicates, and files left
//! untouched for 90 days. Only suggestions: nothing happens until the user
//! moves them to the Trash, and Undo brings them back.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::dupes::{self, HashCache};
use crate::index::Index;

pub const UNTOUCHED_DAYS: i64 = 90;

/// What the system's package database says. A trait so tests can stand in.
pub trait PackageDb: Send + Sync {
    /// (package, version) of a `.deb` file.
    fn deb_fields(&self, deb: &Path) -> Option<(String, String)>;
    /// The installed version of a package, if it is installed.
    fn installed(&self, package: &str) -> Option<String>;
    /// True if version `a` is the same as or newer than `b`.
    fn at_least(&self, a: &str, b: &str) -> bool;
}

/// dpkg, read-only: `dpkg-deb -f`, `dpkg-query -W` and `dpkg --compare-versions`.
pub struct Dpkg;

fn output(program: &str, args: &[&str]) -> Option<String> {
    let out = match Command::new(program).args(args).output() {
        Ok(out) => out,
        Err(error) => {
            glib::g_warning!("steward", "could not run {program}: {error}");
            return None;
        }
    };
    // A failure is an answer too ("not installed"), not an error.
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

impl PackageDb for Dpkg {
    fn deb_fields(&self, deb: &Path) -> Option<(String, String)> {
        let path = deb.to_str()?;
        let text = output("dpkg-deb", &["-f", path, "Package", "Version"])?;
        let mut package = None;
        let mut version = None;
        for line in text.lines() {
            if let Some(v) = line.strip_prefix("Package:") {
                package = Some(v.trim().to_string());
            } else if let Some(v) = line.strip_prefix("Version:") {
                version = Some(v.trim().to_string());
            }
        }
        Some((package?, version?))
    }

    fn installed(&self, package: &str) -> Option<String> {
        let text = output(
            "dpkg-query",
            &["-W", "-f=${db:Status-Status} ${Version}", package],
        )?;
        let (status, version) = text.split_once(' ')?;
        (status == "installed" && !version.is_empty()).then(|| version.to_string())
    }

    fn at_least(&self, a: &str, b: &str) -> bool {
        Command::new("dpkg")
            .args(["--compare-versions", a, "ge", b])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub path: PathBuf,
    pub bytes: u64,
    /// Why, in a few words: "photo-tool 3.2.0 is installed", "also in ~/Documents".
    pub why: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Suggestions {
    pub installers: Vec<Item>,
    pub duplicates: Vec<Item>,
    pub untouched: Vec<Item>,
    /// False when the mount does not record access times (`noatime`):
    /// "untouched" then means not changed rather than not opened.
    pub atime_trusted: bool,
}

/// True unless the filesystem holding `path` is mounted `noatime`.
pub fn atime_trusted(path: &Path) -> bool {
    let Ok(mounts) = std::fs::read_to_string("/proc/self/mountinfo") else {
        return true;
    };
    let mut best: Option<(usize, bool)> = None;
    for line in mounts.lines() {
        let fields: Vec<&str> = line.split(' ').collect();
        let (Some(point), Some(options)) = (fields.get(4), fields.get(5)) else {
            continue;
        };
        if path.starts_with(point) && best.is_none_or(|(len, _)| point.len() > len) {
            best = Some((point.len(), !options.split(',').any(|o| o == "noatime")));
        }
    }
    best.map(|(_, trusted)| trusted).unwrap_or(true)
}

fn display(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

/// Suggestions for the loose files of `folder` as of `now`.
pub fn downloads(
    folder: &Path,
    now: i64,
    packages: &dyn PackageDb,
    index: Option<&Index>,
    cache: &HashCache,
) -> Suggestions {
    let home = glib::home_dir();
    let mut out = Suggestions {
        atime_trusted: atime_trusted(folder),
        ..Default::default()
    };
    let Ok(entries) = std::fs::read_dir(folder) else {
        return out;
    };
    let mut files: Vec<(PathBuf, std::fs::Metadata)> = entries
        .flatten()
        .filter_map(|e| {
            let meta = e.path().symlink_metadata().ok()?;
            (meta.is_file() && !e.file_name().to_string_lossy().starts_with('.'))
                .then(|| (e.path(), meta))
        })
        .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));

    for (path, meta) in &files {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        // Installers for apps already installed. AppImages are the app
        // itself, never an installer (DESIGN.md §4.2).
        if name.ends_with(".deb")
            && let Some((package, version)) = packages.deb_fields(path)
            && let Some(installed) = packages.installed(&package)
            && packages.at_least(&installed, &version)
        {
            out.installers.push(Item {
                path: path.clone(),
                bytes: meta.len(),
                why: format!("{package} {installed} is installed"),
            });
        }
        // Untouched: neither opened (when access times are kept) nor changed.
        let last = if out.atime_trusted {
            meta.atime().max(meta.mtime())
        } else {
            meta.mtime()
        };
        if now - last >= UNTOUCHED_DAYS * 86_400 {
            out.untouched.push(Item {
                path: path.clone(),
                bytes: meta.len(),
                why: format!("{} days", (now - last) / 86_400),
            });
        }
    }

    // Likely duplicates: a loose file with an identical copy elsewhere (or a
    // second one here). The copy suggested is the one in this folder; of
    // several here, all but the oldest.
    let mut candidates: Vec<PathBuf> = files.iter().map(|(p, _)| p.clone()).collect();
    if let Some(index) = index {
        for (_, meta) in &files {
            if meta.len() > 0 {
                candidates.extend(index.files_of_size(meta.len()));
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    for group in dupes::find(&candidates, cache, &|| true) {
        let here: Vec<&PathBuf> = group
            .iter()
            .filter(|p| p.parent() == Some(folder))
            .collect();
        let elsewhere: Vec<&PathBuf> = group
            .iter()
            .filter(|p| p.parent() != Some(folder))
            .collect();
        let keep: Option<&PathBuf> = if elsewhere.is_empty() {
            here.iter().copied().min_by_key(|p| {
                (
                    p.symlink_metadata().map(|m| m.mtime()).unwrap_or(0),
                    p.as_os_str().len(),
                )
            })
        } else {
            None
        };
        for path in here {
            if Some(path) == keep {
                continue;
            }
            let other = elsewhere.first().copied().or(keep).unwrap_or(path);
            out.duplicates.push(Item {
                path: path.clone(),
                bytes: path.symlink_metadata().map(|m| m.len()).unwrap_or(0),
                why: format!(
                    "same as {} in {}",
                    other
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    display(other.parent().unwrap_or(folder), &home)
                ),
            });
        }
    }
    out
}
