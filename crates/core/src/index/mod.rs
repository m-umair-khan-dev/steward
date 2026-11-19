//! The file index (`index.db`, DESIGN.md §7.1, Phase 3): every file and
//! folder under the indexed roots, with folder sizes and counts rolled up and
//! a trigram index on names.
//!
//! `cairn-indexer` is the only writer ([`Index::open`]); the app opens it
//! read-only ([`Index::open_read_only`]).

mod crawl;
mod query;

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension, params};

pub use crawl::{Control, CrawlStats, Rules};
pub use query::{Hit, rank};

/// Bump when the schema changes: an index with another version is rebuilt.
pub const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = "
CREATE TABLE entry (
    id INTEGER PRIMARY KEY,
    parent_id INTEGER,
    name TEXT NOT NULL,
    path TEXT NOT NULL UNIQUE,
    is_dir INTEGER NOT NULL,
    hidden INTEGER NOT NULL,
    size INTEGER NOT NULL,
    mtime INTEGER NOT NULL,
    kind TEXT NOT NULL,
    tree_size INTEGER NOT NULL DEFAULT 0,   -- folders: bytes of every file inside
    tree_files INTEGER NOT NULL DEFAULT 0,  -- folders: files inside, at any depth
    child_count INTEGER NOT NULL DEFAULT 0, -- folders: visible items directly inside
    scanned_at INTEGER NOT NULL
);
CREATE INDEX entry_parent ON entry (parent_id);
CREATE INDEX entry_name ON entry (name COLLATE NOCASE);
CREATE VIRTUAL TABLE name_fts USING fts5 (name, content='entry', content_rowid='id', tokenize='trigram');
CREATE TRIGGER entry_ai AFTER INSERT ON entry BEGIN
    INSERT INTO name_fts (rowid, name) VALUES (new.id, new.name);
END;
CREATE TRIGGER entry_ad AFTER DELETE ON entry BEGIN
    INSERT INTO name_fts (name_fts, rowid, name) VALUES ('delete', old.id, old.name);
END;
CREATE TABLE meta (key TEXT PRIMARY KEY, value INTEGER);
";

/// A folder's numbers, as its card shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FolderStats {
    /// Visible items directly inside.
    pub items: u64,
    /// Bytes of every file inside, at any depth (apparent size).
    pub bytes: u64,
    /// Files inside, at any depth.
    pub files: u64,
}

/// One line of a folder's two-level outline (preview pane).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeLine {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub depth: u32,
    pub items: u64,
}

pub struct Index {
    pub(crate) conn: Connection,
}

pub(crate) fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

impl Index {
    /// `$XDG_CACHE_HOME/cairn/index.db`. Deleting it is safe: it is rebuilt.
    pub fn default_path() -> PathBuf {
        glib::user_cache_dir().join("cairn").join("index.db")
    }

    /// Opens for writing (the indexer). A file from another schema version
    /// is emptied and rebuilt.
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir); // E2-13 allow: the index's own folder, not user files
        }
        let conn = Connection::open(path)?;
        Self::setup(conn)
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Self::setup(Connection::open_in_memory()?)
    }

    fn setup(conn: Connection) -> rusqlite::Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version != SCHEMA_VERSION {
            conn.execute_batch(
                "DROP TRIGGER IF EXISTS entry_ai; DROP TRIGGER IF EXISTS entry_ad;
                 DROP TABLE IF EXISTS name_fts; DROP TABLE IF EXISTS entry; DROP TABLE IF EXISTS meta;",
            )?;
            conn.execute_batch(SCHEMA)?;
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        Ok(Self { conn })
    }

    /// Opens for reading (the app). Fails if there is no index yet.
    pub fn open_read_only(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version != SCHEMA_VERSION {
            return Err(rusqlite::Error::InvalidQuery);
        }
        Ok(Self { conn })
    }

    pub(crate) fn meta(&self, key: &str) -> i64 {
        self.conn
            .query_row("SELECT value FROM meta WHERE key = ?1", params![key], |r| {
                r.get(0)
            })
            .optional()
            .ok()
            .flatten()
            .unwrap_or(0)
    }

    pub(crate) fn set_meta(&self, key: &str, value: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// True once a full crawl has finished at least once.
    pub fn is_complete(&self) -> bool {
        self.meta("complete") == 1
    }

    /// (entries, regular files) in the index.
    pub fn counts(&self) -> (u64, u64) {
        self.conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(is_dir = 0 AND kind != 'link'), 0) FROM entry",
                [],
                |r| Ok((r.get::<_, i64>(0)? as u64, r.get::<_, i64>(1)? as u64)),
            )
            .unwrap_or((0, 0))
    }

    pub fn folder_stats(&self, path: &Path) -> Option<FolderStats> {
        self.conn
            .query_row(
                "SELECT child_count, tree_size, tree_files FROM entry WHERE path = ?1 AND is_dir = 1",
                params![path_text(path)],
                |r| {
                    Ok(FolderStats {
                        items: r.get::<_, i64>(0)? as u64,
                        bytes: r.get::<_, i64>(1)? as u64,
                        files: r.get::<_, i64>(2)? as u64,
                    })
                },
            )
            .optional()
            .ok()
            .flatten()
    }

    /// A folder's visible contents two levels deep, folders first, for the
    /// preview pane. At most `limit` lines.
    pub fn outline(&self, path: &Path, limit: usize) -> Vec<TreeLine> {
        let mut out = Vec::new();
        let Some(id) = self.id_of(path) else {
            return out;
        };
        self.outline_into(id, 0, limit, &mut out);
        out
    }

    fn outline_into(&self, id: i64, depth: u32, limit: usize, out: &mut Vec<TreeLine>) {
        let Ok(mut stmt) = self.conn.prepare_cached(
            "SELECT id, name, path, is_dir, child_count FROM entry
             WHERE parent_id = ?1 AND hidden = 0 ORDER BY is_dir DESC, name COLLATE NOCASE",
        ) else {
            return;
        };
        let rows: Vec<(i64, TreeLine)> = stmt
            .query_map(params![id], |r| {
                Ok((
                    r.get(0)?,
                    TreeLine {
                        name: r.get(1)?,
                        path: PathBuf::from(r.get::<_, String>(2)?),
                        is_dir: r.get(3)?,
                        depth,
                        items: r.get::<_, i64>(4)? as u64,
                    },
                ))
            })
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default();
        for (child, line) in rows {
            if out.len() >= limit {
                return;
            }
            let recurse = line.is_dir && depth == 0;
            out.push(line);
            if recurse {
                self.outline_into(child, depth + 1, limit, out);
            }
        }
    }

    /// Fills the index with `n` made-up entries (1,000 per folder) for
    /// timing searches (scenario E3-10). Not for real use.
    #[doc(hidden)]
    pub fn fill_synthetic(&mut self, n: u64) -> rusqlite::Result<()> {
        const WORDS: [&str; 24] = [
            "report",
            "invoice",
            "budget",
            "draft",
            "photo",
            "IMG",
            "notes",
            "contract",
            "deck",
            "logo",
            "screenshot",
            "final",
            "tax",
            "holiday",
            "readme",
            "export",
            "summary",
            "plan",
            "scan",
            "receipt",
            "meeting",
            "design",
            "backup",
            "q3",
        ];
        const EXTS: [&str; 8] = ["pdf", "png", "jpg", "txt", "docx", "md", "zip", "mp4"];
        let tx = self.conn.transaction()?;
        {
            let mut folder = tx.prepare(
                "INSERT INTO entry (parent_id, name, path, is_dir, hidden, size, mtime, kind, scanned_at)
                 VALUES (NULL, ?1, ?2, 1, 0, 0, ?3, 'folders', 0)",
            )?;
            let mut file = tx.prepare(
                "INSERT INTO entry (parent_id, name, path, is_dir, hidden, size, mtime, kind, scanned_at)
                 VALUES (?1, ?2, ?3, 0, 0, ?4, ?5, 'other', 0)",
            )?;
            let mut parent = 0i64;
            let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
            for i in 0..n {
                // A small xorshift: varied but repeatable names.
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                if i % 1000 == 0 {
                    let name = format!("{} {}", WORDS[(seed % 24) as usize], i / 1000);
                    folder.execute(params![
                        name,
                        format!("/data/{name}"),
                        (seed % 1_000_000) as i64
                    ])?;
                    parent = tx.last_insert_rowid();
                }
                let name = format!(
                    "{}-{}-{}.{}",
                    WORDS[(seed % 24) as usize],
                    WORDS[((seed >> 8) % 24) as usize],
                    (seed >> 16) % 100_000,
                    EXTS[((seed >> 40) % 8) as usize]
                );
                file.execute(params![
                    parent,
                    name,
                    format!("/data/{parent}/{i}-{name}"),
                    ((seed >> 20) % 10_000_000) as i64,
                    (seed % 1_700_000_000) as i64
                ])?;
            }
        }
        tx.commit()
    }

    /// Every indexed folder at or under `path`, shallowest first (the
    /// indexer watches them).
    pub fn folders_under(&self, path: &Path) -> Vec<PathBuf> {
        let p = path_text(path);
        let mut folders: Vec<PathBuf> = self
            .conn
            .prepare_cached(
                "SELECT path FROM entry WHERE is_dir = 1 AND (path = ?1 OR (path > ?2 AND path < ?3))",
            )
            .and_then(|mut s| {
                s.query_map(params![p, format!("{p}/"), format!("{p}0")], |r| r.get::<_, String>(0))
                    .map(|rows| rows.flatten().map(PathBuf::from).collect())
            })
            .unwrap_or_default();
        folders.sort_by_key(|f| f.components().count());
        folders
    }

    pub(crate) fn id_of(&self, path: &Path) -> Option<i64> {
        self.conn
            .query_row(
                "SELECT id FROM entry WHERE path = ?1",
                params![path_text(path)],
                |r| r.get(0),
            )
            .optional()
            .ok()
            .flatten()
    }
}
