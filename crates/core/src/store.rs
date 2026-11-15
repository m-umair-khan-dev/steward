//! `app.db`: the user's own data (DESIGN.md §7.1). Phase 1 keeps the sidebar
//! layout and per-folder view choices here. Migrations run on open.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

/// Schema changes, in order. Never edit a released migration; add a new one.
const MIGRATIONS: &[&str] = &[
    // 1: sidebar layout and per-folder view memory.
    "CREATE TABLE sidebar_entry (
        key TEXT PRIMARY KEY,
        position INTEGER,
        hidden INTEGER NOT NULL DEFAULT 0
     );
     CREATE TABLE folder_view (
        uri TEXT PRIMARY KEY,
        view TEXT,
        sort TEXT,
        descending INTEGER,
        group_by TEXT,
        updated_at INTEGER NOT NULL
     );",
];

/// How one folder was last shown. `None` fields fall back to the defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FolderView {
    pub view: Option<String>,
    pub sort: Option<String>,
    pub descending: Option<bool>,
    pub group_by: Option<String>,
}

/// A stored sidebar choice for one entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarChoice {
    pub key: String,
    pub position: Option<i64>,
    pub hidden: bool,
}

pub struct Store {
    conn: Connection,
}

impl Store {
    /// The default location: `$XDG_DATA_HOME/cairn/app.db`.
    pub fn default_path() -> PathBuf {
        glib::user_data_dir().join("cairn").join("app.db")
    }

    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let mut store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        let mut store = Self { conn: Connection::open_in_memory()? };
        store.migrate()?;
        Ok(store)
    }

    pub fn schema_version(&self) -> rusqlite::Result<i64> {
        self.conn.pragma_query_value(None, "user_version", |row| row.get(0))
    }

    fn migrate(&mut self) -> rusqlite::Result<()> {
        let current = self.schema_version()? as usize;
        for (index, sql) in MIGRATIONS.iter().enumerate().skip(current) {
            let tx = self.conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (index + 1) as i64)?;
            tx.commit()?;
        }
        Ok(())
    }

    // --- sidebar -------------------------------------------------------------

    pub fn sidebar_choices(&self) -> rusqlite::Result<Vec<SidebarChoice>> {
        let mut stmt = self.conn.prepare("SELECT key, position, hidden FROM sidebar_entry")?;
        let rows = stmt.query_map([], |row| {
            Ok(SidebarChoice { key: row.get(0)?, position: row.get(1)?, hidden: row.get(2)? })
        })?;
        rows.collect()
    }

    pub fn set_sidebar_hidden(&self, key: &str, hidden: bool) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO sidebar_entry (key, hidden) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET hidden = excluded.hidden",
            params![key, hidden],
        )?;
        Ok(())
    }

    /// Stores the order of the given keys (position = index).
    pub fn set_sidebar_order(&mut self, keys: &[String]) -> rusqlite::Result<()> {
        let tx = self.conn.transaction()?;
        for (position, key) in keys.iter().enumerate() {
            tx.execute(
                "INSERT INTO sidebar_entry (key, position) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET position = excluded.position",
                params![key, position as i64],
            )?;
        }
        tx.commit()
    }

    pub fn reset_sidebar(&self) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM sidebar_entry", [])?;
        Ok(())
    }

    // --- per-folder view memory ------------------------------------------------

    pub fn folder_view(&self, uri: &str) -> rusqlite::Result<Option<FolderView>> {
        self.conn
            .query_row(
                "SELECT view, sort, descending, group_by FROM folder_view WHERE uri = ?1",
                params![uri],
                |row| {
                    Ok(FolderView {
                        view: row.get(0)?,
                        sort: row.get(1)?,
                        descending: row.get(2)?,
                        group_by: row.get(3)?,
                    })
                },
            )
            .optional()
    }

    pub fn set_folder_view(&self, uri: &str, view: &FolderView, now: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO folder_view (uri, view, sort, descending, group_by, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(uri) DO UPDATE SET view = excluded.view, sort = excluded.sort,
               descending = excluded.descending, group_by = excluded.group_by,
               updated_at = excluded.updated_at",
            params![uri, view.view, view.sort, view.descending, view.group_by, now],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_set_the_schema_version() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.schema_version().unwrap(), MIGRATIONS.len() as i64);
    }

    #[test]
    fn reopening_does_not_rerun_migrations() {
        let path = std::env::temp_dir().join(format!("cairn-store-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let store = Store::open(&path).unwrap();
            store.set_sidebar_hidden("place:music", true).unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(store.sidebar_choices().unwrap()[0].hidden, true);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn sidebar_hide_order_and_reset() {
        let mut store = Store::open_in_memory().unwrap();
        store.set_sidebar_hidden("place:music", true).unwrap();
        store.set_sidebar_order(&["place:home".into(), "place:music".into()]).unwrap();
        let mut choices = store.sidebar_choices().unwrap();
        choices.sort_by(|a, b| a.key.cmp(&b.key));
        assert_eq!(choices[0], SidebarChoice { key: "place:home".into(), position: Some(0), hidden: false });
        assert_eq!(choices[1], SidebarChoice { key: "place:music".into(), position: Some(1), hidden: true });
        store.reset_sidebar().unwrap();
        assert!(store.sidebar_choices().unwrap().is_empty());
    }

    #[test]
    fn folder_view_round_trip() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.folder_view("file:///x").unwrap(), None);
        let view = FolderView {
            view: Some("list".into()),
            sort: Some("size".into()),
            descending: Some(true),
            group_by: Some("kind".into()),
        };
        store.set_folder_view("file:///x", &view, 1).unwrap();
        assert_eq!(store.folder_view("file:///x").unwrap(), Some(view));
    }
}
