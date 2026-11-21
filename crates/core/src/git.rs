//! The Git chip in the path bar (P7-T01, DESIGN.md §4.2): the branch of the
//! repository a folder is in, and how many changes are not committed. Only
//! reads: the repository is byte-for-byte unchanged afterwards (E7-01). Any
//! problem (not a repository, a corrupt `.git`) means no chip, never an error.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitInfo {
    /// The repository's top folder.
    pub root: PathBuf,
    /// The branch, or a short commit id when HEAD is detached.
    pub branch: String,
    /// Changed, added, deleted and new (untracked) files.
    pub changes: usize,
}

/// The repository that holds `folder`, if any.
pub fn info(folder: &Path) -> Option<GitInfo> {
    let repo = gix::discover(folder).ok()?;
    let root = repo.workdir()?.to_path_buf();
    let branch = match repo.head_name().ok()? {
        Some(name) => name.shorten().to_string(),
        None => repo.head_id().ok()?.shorten_or_id().to_string(),
    };
    let changes = repo
        .status(gix::progress::Discard)
        .ok()?
        .untracked_files(gix::status::UntrackedFiles::Files)
        .into_iter(None)
        .ok()?
        .filter_map(Result::ok)
        .count();
    Some(GitInfo {
        root,
        branch,
        changes,
    })
}
