//! Search inside files (P7-T09): which files still need their text read,
//! storing it, and finding a word with the line it is on.
//!
//! Only files under the given folders are read (Home by default: reading
//! contents is off for folders outside Home, DESIGN.md §4.2). The indexer is
//! the only writer; extraction itself happens in a sandbox (`extract`).

use std::path::{Path, PathBuf};

use rusqlite::params;

use super::Index;
use super::query::{COLUMNS, Hit, hit};
use crate::extract;

/// A file whose text is missing or older than the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    pub id: i64,
    pub path: PathBuf,
    pub mtime: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentHit {
    pub hit: Hit,
    /// The line with the word, shortened around it.
    pub line: String,
    /// Where the word is in `line` (bytes).
    pub at: std::ops::Range<usize>,
}

impl Index {
    /// Up to `limit` files under `folders` whose text should be (re)read.
    /// `wanted` says which files the extractor can read at all.
    pub fn content_pending(
        &self,
        folders: &[PathBuf],
        wanted: &dyn Fn(&Path) -> bool,
        limit: usize,
    ) -> Vec<Pending> {
        let mut out = Vec::new();
        let Ok(mut statement) = self.conn.prepare_cached(
            "SELECT e.id, e.path, e.mtime FROM entry e
             LEFT JOIN content_state c ON c.entry_id = e.id
             WHERE e.is_dir = 0 AND e.hidden = 0 AND e.size <= ?2
               AND (e.path = ?1 OR substr(e.path, 1, length(?1) + 1) = ?1 || '/')
               AND (c.mtime IS NULL OR c.mtime != e.mtime)",
        ) else {
            return out;
        };
        for folder in folders {
            let folder = folder.to_string_lossy().trim_end_matches('/').to_string();
            let rows = statement.query_map(params![folder, extract::SIZE_LIMIT as i64], |r| {
                Ok(Pending {
                    id: r.get(0)?,
                    path: PathBuf::from(r.get::<_, String>(1)?),
                    mtime: r.get(2)?,
                })
            });
            let Ok(rows) = rows else { continue };
            for pending in rows.flatten() {
                if wanted(&pending.path) {
                    out.push(pending);
                    if out.len() >= limit {
                        return out;
                    }
                }
            }
        }
        out
    }

    /// Stores what extraction gave for one file.
    pub fn content_store(
        &mut self,
        pending: &Pending,
        outcome: &extract::Outcome,
    ) -> rusqlite::Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM content_fts WHERE rowid = ?1", [pending.id])?;
        if let extract::Outcome::Text(text) = outcome
            && !text.trim().is_empty()
        {
            tx.execute(
                "INSERT INTO content_fts (rowid, body) VALUES (?1, ?2)",
                params![pending.id, text],
            )?;
        }
        tx.execute(
            "INSERT INTO content_state (entry_id, mtime, status) VALUES (?1, ?2, ?3)
             ON CONFLICT(entry_id) DO UPDATE SET mtime = excluded.mtime, status = excluded.status",
            params![pending.id, pending.mtime, outcome.status()],
        )?;
        tx.commit()
    }

    /// Files read so far, by outcome: (ok, failed, timeout).
    pub fn content_counts(&self) -> (u64, u64, u64) {
        let count = |status: &str| -> u64 {
            self.conn
                .query_row(
                    "SELECT COUNT(*) FROM content_state WHERE status = ?1",
                    [status],
                    |r| r.get::<_, i64>(0),
                )
                .map(|n| n as u64)
                .unwrap_or(0)
        };
        (count("ok"), count("failed"), count("timeout"))
    }

    /// How extraction went for a file ("ok", "failed", "timeout"), if it ran.
    pub fn content_status(&self, path: &Path) -> Option<String> {
        self.conn
            .query_row(
                "SELECT c.status FROM content_state c JOIN entry e ON e.id = c.entry_id WHERE e.path = ?1",
                [path.to_string_lossy()],
                |r| r.get(0),
            )
            .ok()
    }

    /// Files whose text contains every word of `query` (word prefixes),
    /// each with the line the first word is on.
    pub fn search_content(&self, query: &str, limit: usize) -> Vec<ContentHit> {
        let terms: Vec<String> = extract::words(query)
            .map(|w| format!("\"{}\"*", w.replace('"', "\"\"")))
            .collect();
        if terms.is_empty() {
            return Vec::new();
        }
        let Ok(mut statement) = self.conn.prepare_cached(&format!(
            "SELECT {COLUMNS}, content_fts.body FROM content_fts
             JOIN entry e ON e.id = content_fts.rowid
             WHERE content_fts MATCH ?1 AND e.hidden = 0
             ORDER BY content_fts.rank LIMIT ?2"
        )) else {
            return Vec::new();
        };
        let rows = statement.query_map(params![terms.join(" "), limit as i64], |r| {
            Ok((hit(r)?, r.get::<_, String>(8)?))
        });
        let Ok(rows) = rows else { return Vec::new() };
        rows.flatten()
            .filter_map(|(hit, body)| {
                let (line, at) = extract::matching_line(&body, query)?;
                Some(ContentHit { hit, line, at })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::Rules;

    #[test]
    fn pending_store_and_search() {
        let dir = std::env::temp_dir().join(format!("cairn-content-{}", std::process::id()));
        let home = dir.join("home");
        std::fs::create_dir_all(home.join("docs")).unwrap();
        std::fs::create_dir_all(dir.join("elsewhere")).unwrap();
        std::fs::write(
            home.join("docs/plan.txt"),
            "intro\nThe Zephyrine budget is final\n",
        )
        .unwrap();
        std::fs::write(home.join("docs/photo.jpg"), "x").unwrap();
        std::fs::write(dir.join("elsewhere/out.txt"), "zephyrine").unwrap();
        let mut index = Index::open_in_memory().unwrap();
        index.crawl(&dir, &Rules::default(), &|| true).unwrap();

        let wanted = |p: &Path| extract::format_of(p).is_some();
        let pending = index.content_pending(std::slice::from_ref(&home), &wanted, 10);
        assert_eq!(
            pending.iter().map(|p| p.path.clone()).collect::<Vec<_>>(),
            [home.join("docs/plan.txt")],
            "only readable files, only under the given folders"
        );
        let text = std::fs::read_to_string(&pending[0].path).unwrap();
        index
            .content_store(&pending[0], &extract::Outcome::Text(text))
            .unwrap();
        assert!(
            index
                .content_pending(std::slice::from_ref(&home), &wanted, 10)
                .is_empty(),
            "read once"
        );
        assert_eq!(
            index.content_status(&home.join("docs/plan.txt")).as_deref(),
            Some("ok")
        );

        let found = index.search_content("zephyr", 10);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].hit.name, "plan.txt");
        assert_eq!(found[0].line, "The Zephyrine budget is final");
        assert_eq!(&found[0].line[found[0].at.clone()], "Zephyr");
        assert!(index.search_content("nothing", 10).is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }
}
