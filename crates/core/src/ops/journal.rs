//! The undo journal in `app.db` (tables `op_batch` and `op_step`, migration 2).
//! Every step is written `pending` before it runs and updated after, so a
//! crash leaves a record of exactly what may have happened.

use std::path::{Path, PathBuf};

use rusqlite::{OptionalExtension, params};

use crate::store::Store;

/// How many batches the journal keeps (DESIGN.md §7.1).
pub const KEEP_BATCHES: i64 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Copy,
    Move,
    Rename,
    Mkdir,
    Trash,
    Restore,
    Erase,
}

impl Action {
    pub fn id(self) -> &'static str {
        match self {
            Action::Copy => "copy",
            Action::Move => "move",
            Action::Rename => "rename",
            Action::Mkdir => "mkdir",
            Action::Trash => "trash",
            Action::Restore => "restore",
            Action::Erase => "erase",
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "copy" => Action::Copy,
            "move" => Action::Move,
            "rename" => Action::Rename,
            "mkdir" => Action::Mkdir,
            "trash" => Action::Trash,
            "restore" => Action::Restore,
            "erase" => Action::Erase,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub id: i64,
    pub seq: i64,
    pub action: Action,
    pub src: Option<PathBuf>,
    pub dst: Option<PathBuf>,
    pub trash: Option<PathBuf>,
    pub state: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Batch {
    pub id: i64,
    pub kind: String,
    pub summary: String,
    pub created_at: i64,
    pub state: String,
    pub undoable: bool,
}

fn uri(path: Option<&Path>) -> Option<String> {
    path.and_then(|p| glib::filename_to_uri(p, None).ok())
        .map(|u| u.to_string())
}

fn path(uri: Option<String>) -> Option<PathBuf> {
    uri.and_then(|u| glib::filename_from_uri(&u).ok())
        .map(|(p, _)| p)
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

const BATCH_COLUMNS: &str = "id, kind, summary, created_at, state, undoable";

fn batch_row(row: &rusqlite::Row) -> rusqlite::Result<Batch> {
    Ok(Batch {
        id: row.get(0)?,
        kind: row.get(1)?,
        summary: row.get(2)?,
        created_at: row.get(3)?,
        state: row.get(4)?,
        undoable: row.get(5)?,
    })
}

impl Store {
    /// Starts a batch in state `running` and drops batches beyond the limit.
    pub fn begin_batch(&self, kind: &str, summary: &str) -> rusqlite::Result<i64> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO op_batch (kind, summary, created_at, state) VALUES (?1, ?2, ?3, 'running')",
            params![kind, summary, now()],
        )?;
        let id = conn.last_insert_rowid();
        conn.execute(
            "DELETE FROM op_batch WHERE id NOT IN (SELECT id FROM op_batch ORDER BY id DESC LIMIT ?1)",
            params![KEEP_BATCHES],
        )?;
        Ok(id)
    }

    pub fn finish_batch(&self, id: i64, state: &str) -> rusqlite::Result<()> {
        self.conn().execute(
            "UPDATE op_batch SET state = ?2 WHERE id = ?1",
            params![id, state],
        )?;
        Ok(())
    }

    pub fn delete_batch(&self, id: i64) -> rusqlite::Result<()> {
        self.conn()
            .execute("DELETE FROM op_batch WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn set_batch_undoable(&self, id: i64, undoable: bool) -> rusqlite::Result<()> {
        self.conn().execute(
            "UPDATE op_batch SET undoable = ?2 WHERE id = ?1",
            params![id, undoable],
        )?;
        Ok(())
    }

    /// Records a step as `pending`, before it runs.
    pub fn add_step(
        &self,
        batch: i64,
        seq: i64,
        action: Action,
        src: Option<&Path>,
        dst: Option<&Path>,
    ) -> rusqlite::Result<i64> {
        self.conn().execute(
            "INSERT INTO op_step (batch_id, seq, action, src_uri, dst_uri, state)
             VALUES (?1, ?2, ?3, ?4, ?5, 'pending')",
            params![batch, seq, action.id(), uri(src), uri(dst)],
        )?;
        Ok(self.conn().last_insert_rowid())
    }

    /// Updates a step after it ran (or was undone or redone).
    pub fn set_step(
        &self,
        step: i64,
        state: &str,
        dst: Option<&Path>,
        trash: Option<&Path>,
        error: Option<&str>,
    ) -> rusqlite::Result<()> {
        self.conn().execute(
            "UPDATE op_step SET state = ?2, dst_uri = COALESCE(?3, dst_uri),
                    trash_uri = ?4, error = ?5 WHERE id = ?1",
            params![step, state, uri(dst), uri(trash), error],
        )?;
        Ok(())
    }

    pub fn batch(&self, id: i64) -> rusqlite::Result<Option<Batch>> {
        self.conn()
            .query_row(
                &format!("SELECT {BATCH_COLUMNS} FROM op_batch WHERE id = ?1"),
                params![id],
                batch_row,
            )
            .optional()
    }

    pub fn batches(&self) -> rusqlite::Result<Vec<Batch>> {
        let mut stmt = self
            .conn()
            .prepare(&format!("SELECT {BATCH_COLUMNS} FROM op_batch ORDER BY id"))?;
        let rows = stmt.query_map([], batch_row)?;
        rows.collect()
    }

    pub fn steps(&self, batch: i64) -> rusqlite::Result<Vec<Step>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, seq, action, src_uri, dst_uri, trash_uri, state, error
             FROM op_step WHERE batch_id = ?1 ORDER BY seq",
        )?;
        let rows = stmt.query_map(params![batch], |row| {
            let action: String = row.get(2)?;
            Ok(Step {
                id: row.get(0)?,
                seq: row.get(1)?,
                action: Action::from_id(&action).unwrap_or(Action::Erase),
                src: path(row.get(3)?),
                dst: path(row.get(4)?),
                trash: path(row.get(5)?),
                state: row.get(6)?,
                error: row.get(7)?,
            })
        })?;
        rows.collect()
    }

    /// The batch Undo would revert: the newest finished one with done steps.
    pub fn undo_candidate(&self) -> rusqlite::Result<Option<Batch>> {
        self.conn()
            .query_row(
                &format!(
                    "SELECT {BATCH_COLUMNS} FROM op_batch b
                     WHERE state IN ('done','partial') AND undoable = 1
                       AND EXISTS (SELECT 1 FROM op_step s WHERE s.batch_id = b.id AND s.state = 'done')
                     ORDER BY id DESC LIMIT 1"
                ),
                [],
                batch_row,
            )
            .optional()
    }

    /// The batch Redo would repeat: the oldest undone batch newer than every
    /// finished one (a new action ends the redo chain).
    pub fn redo_candidate(&self) -> rusqlite::Result<Option<Batch>> {
        self.conn()
            .query_row(
                &format!(
                    "SELECT {BATCH_COLUMNS} FROM op_batch
                     WHERE state = 'undone' AND undoable = 1
                       AND id > COALESCE((SELECT MAX(id) FROM op_batch
                                          WHERE state IN ('done','partial','running')), 0)
                     ORDER BY id ASC LIMIT 1"
                ),
                [],
                batch_row,
            )
            .optional()
    }

    /// After a crash: batches left `running` become `partial`. A pending move
    /// whose destination exists and source does not is recorded as done;
    /// other pending steps as failed. Returns the batches that were open.
    pub fn recover(&self) -> rusqlite::Result<Vec<Batch>> {
        let open: Vec<Batch> = self
            .batches()?
            .into_iter()
            .filter(|b| b.state == "running")
            .collect();
        for batch in &open {
            for step in self.steps(batch.id)? {
                if step.state != "pending" {
                    continue;
                }
                let landed = match (&step.src, &step.dst) {
                    (Some(src), Some(dst)) => {
                        dst.symlink_metadata().is_ok()
                            && (step.action == Action::Copy || src.symlink_metadata().is_err())
                    }
                    (None, Some(dst)) => dst.symlink_metadata().is_ok(),
                    _ => false,
                };
                if landed && step.action != Action::Copy {
                    self.set_step(step.id, "done", None, None, None)?;
                } else {
                    // An interrupted copy may have left part of a file: the
                    // step stays failed and its destination is reported.
                    self.set_step(step.id, "failed", None, None, Some("interrupted"))?;
                }
            }
            self.finish_batch(batch.id, "partial")?;
        }
        Ok(open)
    }

    /// The Trash was emptied: batches whose trashed items are gone cannot be
    /// undone any more.
    pub fn forget_trashed(&self, items: &[PathBuf]) -> rusqlite::Result<()> {
        for item in items {
            let Some(item_uri) = uri(Some(item)) else {
                continue;
            };
            self.conn().execute(
                "UPDATE op_batch SET undoable = 0 WHERE id IN
                   (SELECT batch_id FROM op_step WHERE trash_uri = ?1 AND state IN ('done','undone'))",
                params![item_uri],
            )?;
        }
        Ok(())
    }
}
