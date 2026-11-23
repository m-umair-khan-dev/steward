//! Duplicate finder (DESIGN.md §7.1, P4-T04): same size, then the same
//! first 64 KB, then the same BLAKE3 hash of the whole file. Hashes are kept
//! in a cache keyed by path, size and modification time, so a second look
//! only reads files that changed.

use std::collections::HashMap;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

const PARTIAL: usize = 64 * 1024;

/// `$XDG_CACHE_HOME/steward/hashes.db`. Only a cache: safe to delete.
pub fn default_cache_path() -> PathBuf {
    glib::user_cache_dir().join("steward").join("hashes.db")
}

pub struct HashCache {
    conn: Connection,
}

impl HashCache {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir); // E2-13 allow: the cache's own folder, not user files
        }
        Self::setup(Connection::open(path)?)
    }

    pub fn in_memory() -> rusqlite::Result<Self> {
        Self::setup(Connection::open_in_memory()?)
    }

    fn setup(conn: Connection) -> rusqlite::Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS hash (
                path TEXT PRIMARY KEY, size INTEGER NOT NULL, mtime INTEGER NOT NULL,
                partial BLOB, full BLOB)",
        )?;
        Ok(Self { conn })
    }

    fn get(&self, path: &Path, size: u64, mtime: i64, column: &str) -> Option<Vec<u8>> {
        self.conn
            .query_row(
                &format!("SELECT {column} FROM hash WHERE path = ?1 AND size = ?2 AND mtime = ?3"),
                params![path.to_string_lossy(), size as i64, mtime],
                |r| r.get::<_, Option<Vec<u8>>>(0),
            )
            .optional()
            .ok()
            .flatten()
            .flatten()
    }

    fn put(&self, path: &Path, size: u64, mtime: i64, column: &str, hash: &[u8]) {
        let _ = self.conn.execute(
            &format!(
                "INSERT INTO hash (path, size, mtime, {column}) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(path) DO UPDATE SET {column} = excluded.{column},
                    size = excluded.size, mtime = excluded.mtime,
                    partial = CASE WHEN hash.size = excluded.size AND hash.mtime = excluded.mtime
                                   THEN COALESCE(excluded.partial, hash.partial) ELSE excluded.partial END,
                    full = CASE WHEN hash.size = excluded.size AND hash.mtime = excluded.mtime
                                THEN COALESCE(excluded.full, hash.full) ELSE excluded.full END"
            ),
            params![path.to_string_lossy(), size as i64, mtime, hash],
        );
    }
}

/// Opens for reading without touching the access time: hashing a file is
/// not opening it, and "not opened for 90 days" must stay true (Smart
/// Clean, Downloads). O_NOATIME is refused for files of other users; those
/// are opened normally.
fn open_quietly(path: &Path) -> Option<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new() // E2-13 allow: opens for reading only
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOATIME.bits() as i32)
        .open(path)
        .or_else(|_| std::fs::File::open(path))
        .ok()
}

fn hash_file(path: &Path, limit: Option<usize>) -> Option<Vec<u8>> {
    let mut file = open_quietly(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; 1 << 16];
    let mut read = 0usize;
    loop {
        let want = limit.map_or(buffer.len(), |l| (l - read).min(buffer.len()));
        if want == 0 {
            break;
        }
        let n = file.read(&mut buffer[..want]).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        read += n;
    }
    Some(hasher.finalize().as_bytes().to_vec())
}

struct File {
    path: PathBuf,
    size: u64,
    mtime: i64,
}

fn regroup(groups: Vec<Vec<File>>, key: impl Fn(&File) -> Option<Vec<u8>>) -> Vec<Vec<File>> {
    let mut out = Vec::new();
    for group in groups {
        let mut by_key: HashMap<Vec<u8>, Vec<File>> = HashMap::new();
        for file in group {
            if let Some(k) = key(&file) {
                by_key.entry(k).or_default().push(file);
            }
        }
        out.extend(by_key.into_values().filter(|g| g.len() > 1));
    }
    out
}

/// Groups of identical files among `paths` (regular files of one byte or
/// more; links and folders are ignored). Each group is sorted; groups are
/// sorted by their first path. `keep_going` is asked between files.
pub fn find(
    paths: &[PathBuf],
    cache: &HashCache,
    keep_going: &dyn Fn() -> bool,
) -> Vec<Vec<PathBuf>> {
    let mut by_size: HashMap<u64, Vec<File>> = HashMap::new();
    for path in paths {
        let Ok(meta) = path.symlink_metadata() else {
            continue;
        };
        if !meta.is_file() || meta.len() == 0 {
            continue;
        }
        by_size.entry(meta.len()).or_default().push(File {
            path: path.clone(),
            size: meta.len(),
            mtime: meta.mtime(),
        });
    }
    let candidates: Vec<Vec<File>> = by_size.into_values().filter(|g| g.len() > 1).collect();
    let cached = |file: &File, column: &str, limit: Option<usize>| -> Option<Vec<u8>> {
        if !keep_going() {
            return None;
        }
        if let Some(hash) = cache.get(&file.path, file.size, file.mtime, column) {
            return Some(hash);
        }
        let hash = hash_file(&file.path, limit)?;
        cache.put(&file.path, file.size, file.mtime, column, &hash);
        Some(hash)
    };
    let partial = regroup(candidates, |f| cached(f, "partial", Some(PARTIAL)));
    let full = regroup(partial, |f| {
        // Small files: the first 64 KB is the whole file.
        if f.size as usize <= PARTIAL {
            cached(f, "partial", Some(PARTIAL))
        } else {
            cached(f, "full", None)
        }
    });
    let mut groups: Vec<Vec<PathBuf>> = full
        .into_iter()
        .map(|g| {
            let mut paths: Vec<PathBuf> = g.into_iter().map(|f| f.path).collect();
            paths.sort();
            paths
        })
        .collect();
    groups.sort();
    groups
}
