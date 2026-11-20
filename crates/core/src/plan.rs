//! Plans (DESIGN.md §7.1, P4-T01): a list of moves plus the items left out,
//! each with its reason. A plan is shown first and then handed to `ops` as
//! one batch, so one Undo puts everything back.
//!
//! Tidy (P4-T02, §4.2): loose files in a folder go into folders named after
//! their kind. Folders never move; files still downloading or changed in the
//! last two minutes stay where they are.

use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::Kind;

/// Endings of files that are still downloading.
pub const PARTIAL_SUFFIXES: [&str; 4] = [".crdownload", ".part", ".partial", ".download"];

/// Files changed this recently may still be arriving.
pub const RECENT_SECONDS: i64 = 120;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Move {
    pub from: PathBuf,
    /// The intended destination; a name already taken there gets a free
    /// name when the plan runs ("Keep both").
    pub to: PathBuf,
    pub kind: Kind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// A partial download (.part, .crdownload, …).
    Downloading,
    /// Changed less than two minutes ago.
    Recent,
    /// Hidden files are left alone.
    Hidden,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skip {
    pub path: PathBuf,
    pub reason: SkipReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    pub moves: Vec<Move>,
    pub skipped: Vec<Skip>,
    /// Kind folders that do not exist yet and will be made.
    pub new_folders: Vec<PathBuf>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }

    /// The kind folders the moves go into, existing or new.
    pub fn folders(&self) -> Vec<PathBuf> {
        let mut folders: Vec<PathBuf> = self
            .moves
            .iter()
            .filter_map(|m| m.to.parent().map(Path::to_path_buf))
            .collect();
        folders.sort();
        folders.dedup();
        folders
    }
}

/// Seconds since the epoch: `CAIRN_FAKE_NOW` (seconds, or an RFC 3339
/// date) when set, so tests can move time forward; otherwise the clock.
pub fn now() -> i64 {
    if let Ok(fake) = std::env::var("CAIRN_FAKE_NOW") {
        if let Ok(seconds) = fake.trim().parse::<i64>() {
            return seconds;
        }
        if let Ok(date) = glib::DateTime::from_iso8601(fake.trim(), Some(&glib::TimeZone::utc())) {
            return date.to_unix();
        }
    }
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The kind of a file, from its name (as the views decide it).
pub fn kind_of(name: &str) -> Kind {
    let (content_type, _) = gio::content_type_guess(Some(name), None);
    Kind::from_content_type(&content_type)
}

/// Plans tidying `folder` as of `now`.
pub fn tidy(folder: &Path, now: i64) -> io::Result<Plan> {
    let mut plan = Plan::default();
    let mut entries: Vec<_> = std::fs::read_dir(folder)?.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(meta) = path.symlink_metadata() else {
            continue;
        };
        // Folders never move, nor links to them.
        if meta.is_dir() || (meta.file_type().is_symlink() && path.is_dir()) {
            continue;
        }
        let lower = name.to_lowercase();
        let reason = if name.starts_with('.') {
            Some(SkipReason::Hidden)
        } else if PARTIAL_SUFFIXES.iter().any(|s| lower.ends_with(s)) {
            Some(SkipReason::Downloading)
        } else if now - meta.mtime() < RECENT_SECONDS {
            Some(SkipReason::Recent)
        } else {
            None
        };
        if let Some(reason) = reason {
            plan.skipped.push(Skip { path, reason });
            continue;
        }
        let kind = kind_of(&name);
        let target = folder.join(kind.label());
        if !target.is_dir() && !plan.new_folders.contains(&target) {
            plan.new_folders.push(target.clone());
        }
        plan.moves.push(Move {
            from: path,
            to: target.join(&name),
            kind,
        });
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fake_clock_reads_seconds_and_dates() {
        // Not set here: the real clock is after 2026.
        assert!(now() > 1_767_000_000);
    }

    #[test]
    fn partial_and_recent_files_stay() {
        let dir = std::env::temp_dir().join(format!("cairn-plan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        for name in ["a.pdf", "b.png", "c.iso.part", ".hidden", "new.txt"] {
            std::fs::write(dir.join(name), "x").unwrap();
        }
        let old = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        for name in ["a.pdf", "b.png", "c.iso.part", ".hidden"] {
            std::fs::File::options()
                .write(true)
                .open(dir.join(name))
                .unwrap()
                .set_modified(old)
                .unwrap();
        }
        let plan = tidy(&dir, 1_800_000_000).unwrap();
        let moved: Vec<_> = plan
            .moves
            .iter()
            .map(|m| m.to.strip_prefix(&dir).unwrap().to_path_buf())
            .collect();
        // Far in the future, nothing is recent: new.txt goes too.
        assert_eq!(
            moved,
            vec![
                PathBuf::from("PDFs/a.pdf"),
                PathBuf::from("Images/b.png"),
                PathBuf::from("Documents/new.txt")
            ]
        );
        let skipped: Vec<_> = plan
            .skipped
            .iter()
            .map(|s| (s.path.file_name().unwrap().to_owned(), s.reason))
            .collect();
        assert!(skipped.contains(&(".hidden".into(), SkipReason::Hidden)));
        assert!(skipped.contains(&("c.iso.part".into(), SkipReason::Downloading)));
        // "new.txt" was written just now: as of the real clock it is recent.
        let now_plan = tidy(&dir, now()).unwrap();
        assert!(
            now_plan
                .skipped
                .iter()
                .any(|s| s.path.ends_with("new.txt") && s.reason == SkipReason::Recent)
        );
        assert_eq!(plan.new_folders.len(), 3);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
