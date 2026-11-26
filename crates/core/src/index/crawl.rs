//! Writing the index: full crawls (generation-stamped, so whatever was not
//! seen again is removed) and incremental refreshes of one folder, with the
//! size and count roll-up kept right all the way up.

use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use rusqlite::params;

use super::{Index, path_text};
use crate::Kind;

/// What to leave out.
#[derive(Debug, Clone, Default)]
pub struct Rules {
    /// Folders never indexed (and everything inside).
    pub exclude: Vec<PathBuf>,
}

impl Rules {
    /// Never worth indexing: the app's own data, the Trash, every cache
    /// (`~/.cache` is throwaway by definition, and other apps rewrite it all
    /// the time, which kept the indexer busy) and editor extensions, which
    /// update themselves (product owner's decision, 2026-11-26). Free Up
    /// Space measures caches itself and does not need them in the index.
    pub fn for_home(home: &Path) -> Self {
        Self {
            exclude: [
                ".cache",
                ".vscode/extensions",
                ".local/share/steward",
                ".local/share/Trash",
            ]
            .iter()
            .map(|p| home.join(p))
            .collect(),
        }
    }

    pub fn excluded(&self, path: &Path) -> bool {
        self.exclude.iter().any(|e| path.starts_with(e))
    }
}

/// Asked between folders: false stops the crawl (it can be resumed later).
pub type Control<'a> = &'a dyn Fn() -> bool;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CrawlStats {
    pub entries: u64,
    pub files: u64,
    pub bytes: u64,
    /// False if `Control` stopped it before the end.
    pub finished: bool,
}

/// Rows written between commits.
const BATCH: u32 = 2000;

fn kind_of(name: &str, meta: &Metadata) -> &'static str {
    if meta.file_type().is_symlink() {
        return "link";
    }
    if meta.is_dir() {
        return Kind::Folder.id();
    }
    let (content_type, _) = gio::content_type_guess(Some(name), None);
    Kind::from_content_type(&content_type).id()
}

fn visible(name: &str) -> bool {
    !name.starts_with('.')
}

struct Walk<'a> {
    rules: &'a Rules,
    keep_going: Control<'a>,
    generation: i64,
    device: u64,
    pending: u32,
    stats: CrawlStats,
    stopped: bool,
}

impl Index {
    fn next_generation(&self) -> rusqlite::Result<i64> {
        let generation = self.meta("generation") + 1;
        self.set_meta("generation", generation)?;
        Ok(generation)
    }

    fn upsert(
        &self,
        parent: Option<i64>,
        path: &Path,
        meta: &Metadata,
        hidden: bool,
        generation: i64,
    ) -> rusqlite::Result<i64> {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path_text(path));
        let is_link = meta.file_type().is_symlink();
        let kind = kind_of(&name, meta);
        self.conn
            .prepare_cached(
                "INSERT INTO entry (parent_id, name, path, is_dir, hidden, size, mtime, kind, scanned_at, dev, inode)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                 ON CONFLICT(path) DO UPDATE SET parent_id = excluded.parent_id,
                     is_dir = excluded.is_dir, hidden = excluded.hidden, size = excluded.size,
                     mtime = excluded.mtime, kind = excluded.kind, scanned_at = excluded.scanned_at,
                     dev = excluded.dev, inode = excluded.inode
                 RETURNING id",
            )?
            .query_row(
                params![
                    parent,
                    name,
                    path_text(path),
                    meta.is_dir() && !is_link,
                    hidden,
                    if is_link { 0 } else { meta.len() as i64 },
                    meta.mtime(),
                    kind,
                    generation,
                    meta.dev() as i64,
                    meta.ino() as i64
                ],
                |r| r.get(0),
            )
    }

    fn commit_if_due(&self, walk: &mut Walk) -> rusqlite::Result<()> {
        walk.pending += 1;
        if walk.pending >= BATCH {
            walk.pending = 0;
            self.conn.execute_batch("COMMIT; BEGIN")?;
        }
        Ok(())
    }

    /// Indexes `path` and everything under it. Returns (bytes, files).
    fn walk(
        &self,
        walk: &mut Walk,
        path: &Path,
        meta: &Metadata,
        parent: Option<i64>,
        parent_hidden: bool,
    ) -> rusqlite::Result<(u64, u64)> {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let hidden = parent_hidden || !visible(&name);
        let id = self.upsert(parent, path, meta, hidden, walk.generation)?;
        walk.stats.entries += 1;
        self.commit_if_due(walk)?;
        if meta.file_type().is_symlink() {
            return Ok((0, 0));
        }
        if !meta.is_dir() {
            walk.stats.files += 1;
            walk.stats.bytes += meta.len();
            return Ok((meta.len(), 1));
        }
        if !(walk.keep_going)() {
            walk.stopped = true;
            return Ok((0, 0));
        }
        let (mut bytes, mut files, mut items) = (0u64, 0u64, 0u64);
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if walk.stopped {
                    break;
                }
                let child = entry.path();
                let Ok(child_meta) = child.symlink_metadata() else {
                    continue;
                };
                // Other filesystems (removable drives, network shares) are not
                // indexed from here.
                if walk.rules.excluded(&child) || child_meta.dev() != walk.device {
                    continue;
                }
                let (b, f) = self.walk(walk, &child, &child_meta, Some(id), hidden)?;
                bytes += b;
                files += f;
                if visible(&entry.file_name().to_string_lossy()) {
                    items += 1;
                }
            }
        }
        self.conn.execute(
            "UPDATE entry SET tree_size = ?2, tree_files = ?3, child_count = ?4 WHERE id = ?1",
            params![id, bytes as i64, files as i64, items as i64],
        )?;
        Ok((bytes, files))
    }

    /// Crawls `root` completely. Entries under `root` that were not seen
    /// again are removed, unless `keep_going` stopped the crawl early.
    pub fn crawl(
        &mut self,
        root: &Path,
        rules: &Rules,
        keep_going: Control,
    ) -> rusqlite::Result<CrawlStats> {
        let meta = root
            .symlink_metadata()
            .map_err(|_| rusqlite::Error::InvalidPath(root.to_path_buf()))?;
        let generation = self.next_generation()?;
        let parent = root.parent().and_then(|p| self.id_of(p));
        let mut walk = Walk {
            rules,
            keep_going,
            generation,
            device: meta.dev(),
            pending: 0,
            stats: CrawlStats::default(),
            stopped: false,
        };
        self.conn.execute_batch("BEGIN")?;
        let hidden = root
            .components()
            .any(|c| c.as_os_str().to_string_lossy().starts_with('.'));
        let result = self.walk(&mut walk, root, &meta, parent, hidden);
        if result.is_err() {
            let _ = self.conn.execute_batch("ROLLBACK");
            return result.map(|_| walk.stats);
        }
        if !walk.stopped {
            self.remove_stale(root, generation)?;
            self.set_meta("complete", 1)?;
        }
        self.conn.execute_batch("COMMIT")?;
        walk.stats.finished = !walk.stopped;
        Ok(walk.stats)
    }

    fn remove_stale(&self, root: &Path, generation: i64) -> rusqlite::Result<()> {
        let root = path_text(root);
        self.conn.execute(
            "DELETE FROM entry WHERE scanned_at < ?1 AND (path = ?2 OR (path > ?3 AND path < ?4))",
            params![generation, root, format!("{root}/"), format!("{root}0")],
        )?;
        Ok(())
    }

    fn remove_tree(&self, path: &Path) -> rusqlite::Result<()> {
        let p = path_text(path);
        self.conn.execute(
            "DELETE FROM entry WHERE path = ?1 OR (path > ?2 AND path < ?3)",
            params![p, format!("{p}/"), format!("{p}0")],
        )?;
        Ok(())
    }

    /// Recomputes a folder's roll-up from its direct children, then its
    /// parent's, and so on to the top.
    fn roll_up(&self, mut id: i64) -> rusqlite::Result<()> {
        loop {
            self.conn.execute(
                "UPDATE entry SET
                    tree_size = (SELECT COALESCE(SUM(CASE WHEN c.is_dir THEN c.tree_size
                                                          WHEN c.kind = 'link' THEN 0 ELSE c.size END), 0)
                                 FROM entry c WHERE c.parent_id = ?1),
                    tree_files = (SELECT COALESCE(SUM(CASE WHEN c.is_dir THEN c.tree_files
                                                           WHEN c.kind = 'link' THEN 0 ELSE 1 END), 0)
                                  FROM entry c WHERE c.parent_id = ?1),
                    child_count = (SELECT COUNT(*) FROM entry c
                                   WHERE c.parent_id = ?1 AND substr(c.name, 1, 1) != '.')
                 WHERE id = ?1",
                params![id],
            )?;
            let parent: Option<i64> = self.conn.query_row(
                "SELECT parent_id FROM entry WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )?;
            match parent {
                Some(p) => id = p,
                None => return Ok(()),
            }
        }
    }

    /// Brings one folder up to date after a change inside it: new items are
    /// added (new folders crawled completely), gone ones removed, and the
    /// roll-up fixed up to the top. Folders already indexed keep their
    /// contents; their own changes arrive separately.
    pub fn refresh_dir(&mut self, dir: &Path, rules: &Rules) -> rusqlite::Result<()> {
        if rules.excluded(dir) {
            return Ok(());
        }
        let Ok(meta) = dir.symlink_metadata() else {
            return self.remove_path(dir);
        };
        let Some(id) = self.id_of(dir) else {
            // Not indexed yet: the parent (if indexed) picks it up.
            return match dir.parent() {
                Some(parent) if self.id_of(parent).is_some() => self.refresh_dir(parent, rules),
                _ => Ok(()),
            };
        };
        if !meta.is_dir() {
            return dir.parent().map_or(Ok(()), |p| self.refresh_dir(p, rules));
        }
        let generation = self.next_generation()?;
        let hidden: bool =
            self.conn
                .query_row("SELECT hidden FROM entry WHERE id = ?1", params![id], |r| {
                    r.get(0)
                })?;
        let parent: Option<i64> = self.conn.query_row(
            "SELECT parent_id FROM entry WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )?;
        self.conn.execute_batch("BEGIN")?;
        // The folder itself (its time may have changed).
        self.upsert(parent, dir, &meta, hidden, generation)?;
        let keep_going = || true;
        let mut walk = Walk {
            rules,
            keep_going: &keep_going,
            generation,
            device: meta.dev(),
            pending: 0,
            stats: CrawlStats::default(),
            stopped: false,
        };
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let child = entry.path();
                let Ok(child_meta) = child.symlink_metadata() else {
                    continue;
                };
                if rules.excluded(&child) || child_meta.dev() != meta.dev() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                let child_hidden = hidden || !visible(&name);
                let known_dir: bool = self
                    .conn
                    .query_row(
                        "SELECT is_dir FROM entry WHERE path = ?1",
                        params![path_text(&child)],
                        |r| r.get(0),
                    )
                    .unwrap_or(false);
                if child_meta.is_dir() && !child_meta.file_type().is_symlink() && known_dir {
                    self.upsert(Some(id), &child, &child_meta, child_hidden, generation)?;
                } else {
                    self.walk(&mut walk, &child, &child_meta, Some(id), hidden)?;
                }
            }
        }
        // Children not seen again are gone, with everything under them.
        let gone: Vec<String> = self
            .conn
            .prepare("SELECT path FROM entry WHERE parent_id = ?1 AND scanned_at < ?2")?
            .query_map(params![id, generation], |r| r.get(0))?
            .flatten()
            .collect();
        for path in gone {
            self.remove_tree(Path::new(&path))?;
        }
        self.roll_up(id)?;
        self.conn.execute_batch("COMMIT")
    }

    /// Something at `path` is gone: remove it and everything under it.
    pub fn remove_path(&mut self, path: &Path) -> rusqlite::Result<()> {
        let parent = path.parent().and_then(|p| self.id_of(p));
        self.conn.execute_batch("BEGIN")?;
        self.remove_tree(path)?;
        if let Some(parent) = parent {
            self.roll_up(parent)?;
        }
        self.conn.execute_batch("COMMIT")
    }
}

#[cfg(test)]
mod rules_tests {
    use super::*;

    #[test]
    fn home_rules_skip_caches_and_editor_extensions() {
        let home = Path::new("/h");
        let rules = Rules::for_home(home);
        for skipped in ["/h/.cache", "/h/.cache/pip/x", "/h/.cache/steward/index.db",
                        "/h/.vscode/extensions/a/b.js", "/h/.local/share/Trash/files/x"] {
            assert!(rules.excluded(Path::new(skipped)), "{skipped} is skipped");
        }
        for kept in ["/h/Documents/a.txt", "/h/.vscode/settings.json", "/h/.cachet/x",
                     "/h/.config/app.conf"] {
            assert!(!rules.excluded(Path::new(kept)), "{kept} is indexed");
        }
    }
}
