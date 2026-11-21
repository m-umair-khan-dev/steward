//! Reading the index: name search with ranking, and folders by name.
//!
//! Ranking (P3-T08): an exact name first, then names that start with the
//! query, then names with a word that starts with it, then names that merely
//! contain it; within each, the most recently changed first.

use std::path::PathBuf;

use rusqlite::params;

use super::Index;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub kind: String,
    pub size: u64,
    pub mtime: i64,
    /// Folders: visible items directly inside.
    pub items: u64,
    /// Folders: bytes inside, at any depth.
    pub bytes: u64,
}

/// How well `name` matches `query` (lower is better), or None.
pub fn rank(name: &str, query: &str) -> Option<u8> {
    rank_lower(&name.to_lowercase(), &query.to_lowercase())
}

fn rank_lower(name: &str, query: &str) -> Option<u8> {
    if name == query {
        return Some(0);
    }
    if name.starts_with(query) {
        return Some(1);
    }
    let at = name.find(query)?;
    let before = name[..at].chars().next_back();
    if before.is_some_and(|c| !c.is_alphanumeric()) {
        Some(2)
    } else {
        Some(3)
    }
}

pub(crate) const COLUMNS: &str =
    "e.path, e.name, e.is_dir, e.kind, e.size, e.mtime, e.child_count, e.tree_size";

pub(crate) fn hit(r: &rusqlite::Row) -> rusqlite::Result<Hit> {
    Ok(Hit {
        path: PathBuf::from(r.get::<_, String>(0)?),
        name: r.get(1)?,
        is_dir: r.get(2)?,
        kind: r.get(3)?,
        size: r.get::<_, i64>(4)? as u64,
        mtime: r.get(5)?,
        items: r.get::<_, i64>(6)? as u64,
        bytes: r.get::<_, i64>(7)? as u64,
    })
}

/// Candidates read before ranking.
const CANDIDATES: usize = 5000;

impl Index {
    /// Files and folders whose name contains `query`, best first. Hidden
    /// items are left out unless the query itself starts with a dot.
    pub fn search(&self, query: &str, limit: usize) -> Vec<Hit> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        let hidden_ok = query.starts_with('.');
        // Ranking needs only (id, name, time); whole rows are read for the
        // few that are shown.
        let light = |sql: &str, arg: &dyn rusqlite::ToSql| -> Vec<(i64, String, i64)> {
            self.conn
                .prepare_cached(sql)
                .and_then(|mut s| {
                    s.query_map(params![arg, hidden_ok, CANDIDATES as i64], |r| {
                        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                    })
                    .map(|rows| rows.flatten().collect())
                })
                .unwrap_or_default()
        };
        // Names that start with the query, through the name index: these
        // rank high and must never be crowded out by substring matches.
        let pattern = format!(
            "{}%",
            query
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let mut candidates = light(
            "SELECT e.id, e.name, e.mtime FROM entry e
             WHERE e.name LIKE ?1 ESCAPE '\\' AND (?2 OR e.hidden = 0) LIMIT ?3",
            &pattern,
        );
        if query.chars().count() >= 3 {
            // The trigram index finds substrings; the query is quoted as a phrase.
            let phrase = format!("\"{}\"", query.replace('"', "\"\""));
            let seen: std::collections::HashSet<i64> = candidates.iter().map(|c| c.0).collect();
            let substring = light(
                "SELECT e.id, e.name, e.mtime FROM name_fts JOIN entry e ON e.id = name_fts.rowid
                 WHERE name_fts MATCH ?1 AND (?2 OR e.hidden = 0) LIMIT ?3",
                &phrase,
            );
            candidates.extend(substring.into_iter().filter(|c| !seen.contains(&c.0)));
        }
        let lower = query.to_lowercase();
        let mut ranked: Vec<(u8, i64, usize, i64)> = candidates
            .iter()
            .filter_map(|(id, name, mtime)| {
                rank_lower(&name.to_lowercase(), &lower).map(|r| (r, -*mtime, name.len(), *id))
            })
            .collect();
        let keep = limit.min(ranked.len());
        if keep == 0 {
            return Vec::new();
        }
        ranked.select_nth_unstable(keep - 1);
        ranked.truncate(keep);
        ranked.sort_unstable();
        let mut stmt = match self
            .conn
            .prepare_cached(&format!("SELECT {COLUMNS} FROM entry e WHERE e.id = ?1"))
        {
            Ok(stmt) => stmt,
            Err(_) => return Vec::new(),
        };
        ranked
            .iter()
            .filter_map(|(_, _, _, id)| stmt.query_row(params![id], hit).ok())
            .collect()
    }

    /// Visible folders whose name is `name` (any case), then those whose
    /// name starts with it: for typing a folder's name in the path bar.
    pub fn folders_named(&self, name: &str, limit: usize) -> Vec<Hit> {
        let name = name.trim();
        if name.is_empty() {
            return Vec::new();
        }
        let pattern = format!(
            "{}%",
            name.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        let mut hits: Vec<Hit> = self
            .conn
            .prepare_cached(&format!(
                "SELECT {COLUMNS} FROM entry e
                 WHERE e.is_dir = 1 AND e.hidden = 0 AND e.name LIKE ?1 ESCAPE '\\' LIMIT ?2"
            ))
            .and_then(|mut s| {
                s.query_map(params![pattern, CANDIDATES as i64], hit)
                    .map(|rows| rows.flatten().collect())
            })
            .unwrap_or_default();
        hits.sort_by(|a, b| {
            rank(&a.name, name)
                .cmp(&rank(&b.name, name))
                .then(
                    a.path
                        .components()
                        .count()
                        .cmp(&b.path.components().count()),
                )
                .then_with(|| a.path.cmp(&b.path))
        });
        hits.truncate(limit);
        hits
    }
}

#[cfg(test)]
mod tests {
    use super::rank;

    #[test]
    fn ranks_exact_prefix_word_then_anywhere() {
        assert_eq!(rank("Invoices", "invoices"), Some(0));
        assert_eq!(rank("Invoice-0923.pdf", "invo"), Some(1));
        assert_eq!(rank("Acme-Invoice.pdf", "invo"), Some(2));
        assert_eq!(rank("reinvoiced.txt", "invo"), Some(3));
        assert_eq!(rank("notes.txt", "invo"), None);
    }
}
