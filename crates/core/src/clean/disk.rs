//! The disk overview at the top of Smart Clean (P5-T02): the disk that holds
//! the home folder, its free space, and what the used space is made of.
//!
//! Free space is `f_bavail`, as Files and Disk Usage Analyzer count it, so
//! the numbers agree across apps. Parts are apparent sizes; the overview
//! scales them to fit the used space when sparse files make them add up to
//! more.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use super::tree_bytes;
use crate::tools::Runner;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Disk {
    pub size: u64,
    pub free: u64,
    /// The home folder, without the Trash.
    pub home: u64,
    /// Installed software on this disk (apt, Snap, Flatpak); None when the
    /// system's software lives on another disk.
    pub apps: Option<u64>,
    /// The home Trash.
    pub trash: u64,
}

impl Disk {
    pub fn used(&self) -> u64 {
        self.size.saturating_sub(self.free)
    }

    /// Everything used that is not one of the named parts.
    pub fn other(&self) -> u64 {
        self.used()
            .saturating_sub(self.home + self.apps.unwrap_or(0) + self.trash)
    }
}

/// (size, free) of the filesystem holding `path`.
pub fn space(path: &Path) -> Option<(u64, u64)> {
    let stat = rustix::fs::statvfs(path).ok()?;
    Some((stat.f_blocks * stat.f_frsize, stat.f_bavail * stat.f_frsize))
}

/// Bytes of installed software: dpkg's `Installed-Size` for every package,
/// Snap images and system Flatpaks.
pub fn software_bytes(runner: &dyn Runner) -> u64 {
    let apt: u64 = runner
        .run("dpkg-query", &["-W", "-f=${Installed-Size}\n"])
        .output()
        .map(|text| {
            text.lines()
                .filter_map(|l| l.trim().parse::<u64>().ok())
                .sum::<u64>()
                * 1024
        })
        .unwrap_or(0);
    let snaps: u64 = std::fs::read_dir("/var/lib/snapd/snaps")
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| e.metadata().ok())
                .filter(|m| m.is_file())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0);
    apt + snaps + tree_bytes(Path::new("/var/lib/flatpak"))
}

/// Measures the disk that holds `home`. Slow (it walks the home folder):
/// call it off the main thread.
pub fn overview(home: &Path, trash_files: &Path, runner: &dyn Runner) -> Disk {
    let (size, free) = space(home).unwrap_or((0, 0));
    let trash_root: PathBuf = trash_files
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| trash_files.to_path_buf());
    let trash = tree_bytes(trash_files);
    let home_bytes = tree_bytes(home).saturating_sub(tree_bytes(&trash_root));
    let device = |p: &Path| p.metadata().map(|m| m.dev()).ok();
    let apps = (device(Path::new("/usr")) == device(home)).then(|| software_bytes(runner));
    Disk {
        size,
        free,
        home: home_bytes,
        apps,
        trash,
    }
}
