//! The Shelf (P8-T01; DESIGN.md §4.4): numbered pinned folders, Holding (a
//! well that remembers files without moving them, across restarts), and the
//! latest downloads. Pins and Holding are kept in `app.db`; nothing here
//! touches the files themselves (drops go through `ops`).

use std::path::{Path, PathBuf};

use rusqlite::params;

use crate::store::Store;

/// Pinned folders the Shelf shows (keys 1 to 9).
pub const MAX_PINS: usize = 9;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    pub id: i64,
    pub path: PathBuf,
}

impl Store {
    /// Pinned folders, in order.
    pub fn shelf_pins(&self) -> rusqlite::Result<Vec<PathBuf>> {
        let mut statement = self
            .conn()
            .prepare("SELECT path FROM shelf_pin ORDER BY position, id")?;
        statement
            .query_map([], |r| Ok(PathBuf::from(r.get::<_, String>(0)?)))?
            .collect()
    }

    /// Whether the user has ever set the pins (before that, defaults show).
    pub fn shelf_pins_chosen(&self) -> bool {
        self.conn()
            .query_row(
                "SELECT COUNT(*) FROM shelf_meta WHERE key = 'pins-chosen'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n > 0)
            .unwrap_or(false)
    }

    /// Replaces the pinned folders (at most nine, no repeats).
    pub fn set_shelf_pins(&mut self, pins: &[PathBuf]) -> rusqlite::Result<()> {
        let tx = self.conn_mut().transaction()?;
        tx.execute("DELETE FROM shelf_pin", [])?;
        let mut seen: Vec<&PathBuf> = Vec::new();
        for pin in pins {
            if seen.contains(&pin) || seen.len() >= MAX_PINS {
                continue;
            }
            tx.execute(
                "INSERT INTO shelf_pin (path, position) VALUES (?1, ?2)",
                params![pin.to_string_lossy(), seen.len() as i64],
            )?;
            seen.push(pin);
        }
        tx.execute(
            "INSERT OR IGNORE INTO shelf_meta (key, value) VALUES ('pins-chosen', 1)",
            [],
        )?;
        tx.commit()
    }

    /// Files and folders in Holding, oldest first.
    pub fn shelf_held(&self) -> rusqlite::Result<Vec<Held>> {
        let mut statement = self
            .conn()
            .prepare("SELECT id, path FROM shelf_held ORDER BY added_at, id")?;
        statement
            .query_map([], |r| {
                Ok(Held {
                    id: r.get(0)?,
                    path: PathBuf::from(r.get::<_, String>(1)?),
                })
            })?
            .collect()
    }

    /// Puts items in Holding (an item already there stays once).
    pub fn shelf_hold(&mut self, paths: &[PathBuf], now: i64) -> rusqlite::Result<()> {
        let tx = self.conn_mut().transaction()?;
        for path in paths {
            tx.execute(
                "INSERT INTO shelf_held (path, added_at) SELECT ?1, ?2
                 WHERE NOT EXISTS (SELECT 1 FROM shelf_held WHERE path = ?1)",
                params![path.to_string_lossy(), now],
            )?;
        }
        tx.commit()
    }

    /// Takes one item out of Holding (the file itself stays where it is).
    pub fn shelf_unhold(&self, id: i64) -> rusqlite::Result<()> {
        self.conn()
            .execute("DELETE FROM shelf_held WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Empties Holding (the files themselves stay where they are).
    pub fn shelf_clear(&self) -> rusqlite::Result<()> {
        self.conn().execute("DELETE FROM shelf_held", [])?;
        Ok(())
    }
}

/// A download still being written: Firefox, Chromium and others.
pub fn is_partial(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".part", ".crdownload", ".download", ".partial", ".tmp"]
        .iter()
        .any(|s| lower.ends_with(s))
}

/// The newest files in `folder`: no folders, hidden files or partial
/// downloads; newest first.
pub fn latest_files(folder: &Path, count: usize) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            !name.starts_with('.') && !is_partial(&name)
        })
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            Some((meta.modified().ok()?, e.path()))
        })
        .collect();
    files.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    files.into_iter().take(count).map(|(_, p)| p).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_and_holding() {
        let mut store = Store::open_in_memory().unwrap();
        assert!(!store.shelf_pins_chosen());
        let a = PathBuf::from("/h/Documents");
        let b = PathBuf::from("/h/Downloads");
        store
            .set_shelf_pins(&[a.clone(), b.clone(), a.clone()])
            .unwrap();
        assert_eq!(store.shelf_pins().unwrap(), [a.clone(), b.clone()]);
        assert!(store.shelf_pins_chosen());

        store
            .shelf_hold(&[PathBuf::from("/h/x.txt"), PathBuf::from("/h/y.txt")], 1)
            .unwrap();
        store.shelf_hold(&[PathBuf::from("/h/x.txt")], 2).unwrap();
        let held = store.shelf_held().unwrap();
        assert_eq!(
            held.iter().map(|h| h.path.clone()).collect::<Vec<_>>(),
            [PathBuf::from("/h/x.txt"), PathBuf::from("/h/y.txt")]
        );
        store.shelf_unhold(held[0].id).unwrap();
        assert_eq!(store.shelf_held().unwrap().len(), 1);
        store.shelf_clear().unwrap();
        assert!(store.shelf_held().unwrap().is_empty());
    }

    #[test]
    fn latest_files_skip_folders_hidden_and_partial() {
        let dir = std::env::temp_dir().join(format!("steward-shelf-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        for (i, name) in [
            "a.pdf",
            "b.jpg",
            ".hidden",
            "c.zip.part",
            "d.iso.crdownload",
            "e.txt",
        ]
        .iter()
        .enumerate()
        {
            let path = dir.join(name);
            std::fs::write(&path, b"x").unwrap();
            let t = std::time::SystemTime::UNIX_EPOCH
                + std::time::Duration::from_secs(1_000_000 + i as u64 * 10);
            std::fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(t)
                .unwrap();
        }
        let names: Vec<String> = latest_files(&dir, 5)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["e.txt", "b.jpg", "a.pdf"]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
