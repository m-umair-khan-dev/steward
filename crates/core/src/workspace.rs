//! Workspaces (DESIGN.md §4.2, P4-T08, P4-T09): folders and files from
//! anywhere on one page. Nothing moves: a workspace keeps references, each
//! with its path plus filesystem and inode, so an item renamed or moved on
//! the same filesystem is found again. A missing item offers Locate…; an
//! item on a drive that is not plugged in shows the drive's name.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use rusqlite::{OptionalExtension, params};

use crate::index::Index;
use crate::store::Store;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub id: i64,
    pub name: String,
    pub description: String,
    /// A kind id used as its colour ("folders", "archives", "audio" …).
    pub color: String,
    pub position: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: i64,
    pub workspace: i64,
    pub path: PathBuf,
    pub dev: u64,
    pub inode: u64,
    /// The volume it lives on when that is not the home's ("Backup Drive").
    pub volume: Option<String>,
    /// That volume's mount point, to tell "unplugged" from "missing".
    pub mount: Option<PathBuf>,
    pub is_dir: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Where {
    /// At its recorded place.
    Here(PathBuf),
    /// Found again somewhere else (renamed or moved); the record is updated.
    Moved(PathBuf),
    /// Its drive is not plugged in.
    Unplugged(String),
    /// Gone: Locate… can point it at something else.
    Missing,
}

/// The mount point that holds `path` (the longest one in mountinfo).
pub fn mount_point(path: &Path) -> Option<PathBuf> {
    let mounts = std::fs::read_to_string("/proc/self/mountinfo").ok()?;
    mounts
        .lines()
        .filter_map(|line| line.split(' ').nth(4).map(PathBuf::from))
        .filter(|point| path.starts_with(point))
        .max_by_key(|point| point.as_os_str().len())
}

fn now() -> i64 {
    crate::plan::now()
}

fn workspace_row(r: &rusqlite::Row) -> rusqlite::Result<Workspace> {
    Ok(Workspace {
        id: r.get(0)?,
        name: r.get(1)?,
        description: r.get(2)?,
        color: r.get(3)?,
        position: r.get(4)?,
    })
}

fn item_row(r: &rusqlite::Row) -> rusqlite::Result<Item> {
    Ok(Item {
        id: r.get(0)?,
        workspace: r.get(1)?,
        path: PathBuf::from(r.get::<_, String>(2)?),
        dev: r.get::<_, i64>(3)? as u64,
        inode: r.get::<_, i64>(4)? as u64,
        volume: r.get(5)?,
        mount: r.get::<_, Option<String>>(6)?.map(PathBuf::from),
        is_dir: r.get(7)?,
    })
}

const ITEM_COLUMNS: &str = "id, workspace_id, path, dev, inode, volume_label, mount_point, is_dir";

impl Store {
    pub fn workspaces(&self) -> rusqlite::Result<Vec<Workspace>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, name, description, color, position FROM workspace ORDER BY position, id",
        )?;
        let rows = stmt.query_map([], workspace_row)?;
        rows.collect()
    }

    pub fn workspace(&self, id: i64) -> rusqlite::Result<Option<Workspace>> {
        self.conn()
            .query_row(
                "SELECT id, name, description, color, position FROM workspace WHERE id = ?1",
                params![id],
                workspace_row,
            )
            .optional()
    }

    pub fn create_workspace(&self, name: &str, color: &str) -> rusqlite::Result<i64> {
        let position: i64 = self.conn().query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM workspace",
            [],
            |r| r.get(0),
        )?;
        self.conn().execute(
            "INSERT INTO workspace (name, description, color, position, created_at) VALUES (?1, '', ?2, ?3, ?4)",
            params![name, color, position, now()],
        )?;
        Ok(self.conn().last_insert_rowid())
    }

    pub fn update_workspace(
        &self,
        id: i64,
        name: &str,
        description: &str,
        color: &str,
    ) -> rusqlite::Result<()> {
        self.conn().execute(
            "UPDATE workspace SET name = ?2, description = ?3, color = ?4 WHERE id = ?1",
            params![id, name, description, color],
        )?;
        Ok(())
    }

    pub fn delete_workspace(&self, id: i64) -> rusqlite::Result<()> {
        self.conn()
            .execute("DELETE FROM workspace WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn workspace_items(&self, workspace: i64) -> rusqlite::Result<Vec<Item>> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {ITEM_COLUMNS} FROM workspace_item WHERE workspace_id = ?1 ORDER BY position, id"
        ))?;
        let rows = stmt.query_map(params![workspace], item_row)?;
        rows.collect()
    }

    /// Adds `path` (once) with what is needed to find it again later.
    pub fn add_to_workspace(&self, workspace: i64, path: &Path) -> rusqlite::Result<Option<i64>> {
        let Ok(meta) = path.metadata() else {
            return Ok(None);
        };
        let exists: bool = self.conn().query_row(
            "SELECT EXISTS(SELECT 1 FROM workspace_item WHERE workspace_id = ?1 AND path = ?2)",
            params![workspace, path.to_string_lossy()],
            |r| r.get(0),
        )?;
        if exists {
            return Ok(None);
        }
        let home_dev = glib::home_dir().metadata().map(|m| m.dev()).unwrap_or(0);
        let mount = mount_point(path);
        let volume = (meta.dev() != home_dev).then(|| volume_label(path, mount.as_deref()));
        let position: i64 = self.conn().query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM workspace_item WHERE workspace_id = ?1",
            params![workspace],
            |r| r.get(0),
        )?;
        self.conn().execute(
            "INSERT INTO workspace_item (workspace_id, path, dev, inode, volume_label, mount_point, is_dir, position, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                workspace,
                path.to_string_lossy(),
                meta.dev() as i64,
                meta.ino() as i64,
                volume,
                mount.map(|m| m.to_string_lossy().into_owned()),
                meta.is_dir(),
                position,
                now()
            ],
        )?;
        Ok(Some(self.conn().last_insert_rowid()))
    }

    pub fn remove_from_workspace(&self, item: i64) -> rusqlite::Result<()> {
        self.conn()
            .execute("DELETE FROM workspace_item WHERE id = ?1", params![item])?;
        Ok(())
    }

    /// Points an item at `path` (Locate…, or after it was found again).
    pub fn relocate_item(&self, item: i64, path: &Path) -> rusqlite::Result<bool> {
        let Ok(meta) = path.metadata() else {
            return Ok(false);
        };
        self.conn().execute(
            "UPDATE workspace_item SET path = ?2, dev = ?3, inode = ?4, is_dir = ?5 WHERE id = ?1",
            params![
                item,
                path.to_string_lossy(),
                meta.dev() as i64,
                meta.ino() as i64,
                meta.is_dir()
            ],
        )?;
        Ok(true)
    }

    /// Where an item is now; a renamed or moved item is found again by its
    /// inode (first next to where it was, then through the index) and its
    /// record updated.
    pub fn resolve_item(&self, item: &Item, index: Option<&Index>) -> Where {
        if let Ok(meta) = item.path.symlink_metadata()
            && (item.inode == 0 || meta.ino() == item.inode)
        {
            return Where::Here(item.path.clone());
        }
        if let Some(mount) = &item.mount
            && !mount.exists()
        {
            return Where::Unplugged(
                item.volume
                    .clone()
                    .unwrap_or_else(|| mount.display().to_string()),
            );
        }
        let mut found = None;
        if let Some(parent) = item.path.parent()
            && let Ok(entries) = std::fs::read_dir(parent)
        {
            found = entries
                .flatten()
                .find(|e| {
                    e.metadata()
                        .map(|m| m.ino() == item.inode && m.dev() == item.dev)
                        .unwrap_or(false)
                })
                .map(|e| e.path());
        }
        if found.is_none() {
            found = index
                .and_then(|i| i.path_of_inode(item.dev, item.inode))
                .filter(|p| p.exists());
        }
        match found {
            Some(path) => {
                let _ = self.relocate_item(item.id, &path);
                Where::Moved(path)
            }
            None if item.path.exists() => Where::Here(item.path.clone()),
            None => Where::Missing,
        }
    }
}

/// A volume's name for labels: GIO's mount name when it has one, else the
/// mount point's last part.
pub fn volume_label(path: &Path, mount: Option<&Path>) -> String {
    use gio::prelude::*;
    if let Ok(found) = gio::File::for_path(path).find_enclosing_mount(gio::Cancellable::NONE) {
        return found.name().to_string();
    }
    mount
        .and_then(|m| m.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "another drive".into())
}
