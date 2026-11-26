//! Extract Here (P11-T02, GitHub issue #5): unpacks an archive into a new
//! folder beside it, through `bsdtar` (libarchive: ZIP, 7z, RAR 4 and 5,
//! tar with gzip, bzip2, xz or zstd, ISO, CAB).
//!
//! The archive is unpacked into a hidden staging folder first and moved into
//! place when it is complete, so a cancelled or failed extraction leaves
//! nothing behind. If everything in it sits in one top folder, that folder
//! is used as is ("site.zip" holding "site/" gives "site", not "site/site").
//!
//! bsdtar refuses names with ".." and strips a leading "/", and does not
//! write through symbolic links it unpacked: an archive cannot write
//! outside its folder.

use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gio::prelude::*;

use super::fsx;

const BSDTAR: &str = "bsdtar";

/// Endings Extract Here knows, longest first so "a.tar.gz" loses ".tar.gz".
const ENDINGS: &[&str] = &[
    ".tar.gz",
    ".tar.bz2",
    ".tar.xz",
    ".tar.zst",
    ".tar.lz",
    ".tar.lzma",
    ".tgz",
    ".tbz2",
    ".tbz",
    ".txz",
    ".tzst",
    ".tar",
    ".zip",
    ".7z",
    ".rar",
    ".cab",
    ".iso",
    ".xar",
    ".cpio",
    ".lha",
    ".lzh",
];

/// Whether Extract Here offers to unpack this file (by its name).
pub fn is_archive(path: &Path) -> bool {
    ending(&fsx::file_name(path)).is_some()
}

fn ending(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    ENDINGS
        .iter()
        .copied()
        .find(|e| lower.len() > e.len() && lower.ends_with(e))
}

/// The folder an archive unpacks into: its name without the ending.
pub fn folder_name(archive: &Path) -> String {
    let name = fsx::file_name(archive);
    match ending(&name) {
        Some(e) => name[..name.len() - e.len()].to_string(),
        None => format!("{name} contents"),
    }
}

/// Whether the extraction tool is installed (libarchive-tools).
pub fn available() -> bool {
    Command::new(BSDTAR)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// How many entries the archive lists (for progress); 0 if unknown.
pub fn count_entries(archive: &Path) -> u64 {
    Command::new(BSDTAR)
        .arg("-tf")
        .arg(archive)
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| o.stdout.iter().filter(|b| **b == b'\n').count() as u64)
        .unwrap_or(0)
}

/// Unpacks `archive` into a staging folder in `dir`. Returns the item to
/// move into place (the staging folder, or the one folder inside it), the
/// staging folder, and the name to give it. `on_entry` is called with each
/// name as it is written. Nothing is left behind on an error.
pub fn unpack(
    archive: &Path,
    dir: &Path,
    cancel: &gio::Cancellable,
    on_entry: &mut dyn FnMut(&str),
) -> io::Result<(PathBuf, PathBuf, String)> {
    let staging = dir.join(format!(
        ".{}.steward-extract-{}",
        fsx::file_name(archive),
        std::process::id()
    ));
    if fsx::exists(&staging) {
        fsx::remove_tree(&staging)?;
    }
    fsx::create_dir(&staging)?;
    if let Err(error) = run(archive, &staging, cancel, on_entry) {
        let _ = fsx::remove_tree(&staging);
        return Err(error);
    }
    let children: Vec<_> = std::fs::read_dir(&staging)?
        .filter_map(Result::ok)
        .collect();
    match children.as_slice() {
        [] => {
            let _ = fsx::remove_tree(&staging);
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the archive is empty",
            ))
        }
        [only] if only.file_type().is_ok_and(|t| t.is_dir()) => {
            let name = only.file_name().to_string_lossy().into_owned();
            Ok((only.path(), staging, name))
        }
        _ => Ok((staging.clone(), staging, folder_name(archive))),
    }
}

fn run(
    archive: &Path,
    into: &Path,
    cancel: &gio::Cancellable,
    on_entry: &mut dyn FnMut(&str),
) -> io::Result<()> {
    let mut child = Command::new(BSDTAR)
        // -v lists each entry on stderr as "x name" while it is written.
        .args(["-x", "-v", "--no-same-owner", "-f"])
        .arg(archive)
        .arg("-C")
        .arg(into)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => io::Error::new(
                io::ErrorKind::NotFound,
                "the extraction tool (libarchive-tools) is not installed",
            ),
            _ => error,
        })?;
    let stderr = child.stderr.take().expect("stderr is piped");
    // Read the listing on another thread so the wait below can watch Cancel.
    let (lines_tx, lines_rx) = std::sync::mpsc::channel::<String>();
    let problems = Arc::new(Mutex::new(Vec::<String>::new()));
    let finished = Arc::new(AtomicBool::new(false));
    let reader = {
        let (problems, finished) = (problems.clone(), finished.clone());
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                match line.strip_prefix("x ") {
                    Some(name) => {
                        let _ = lines_tx.send(name.to_string());
                    }
                    None => problems.lock().expect("not poisoned").push(line),
                }
            }
            finished.store(true, Ordering::SeqCst);
        })
    };
    let status = loop {
        for name in lines_rx.try_iter() {
            on_entry(&name);
        }
        if cancel.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err(fsx::cancelled());
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        std::thread::sleep(Duration::from_millis(30));
    };
    let _ = reader.join();
    for name in lines_rx.try_iter() {
        on_entry(&name);
    }
    if status.success() {
        return Ok(());
    }
    let problems = problems.lock().expect("not poisoned").join("\n");
    Err(explain(&problems))
}

/// bsdtar's complaint as a sentence a person can act on.
fn explain(problems: &str) -> io::Error {
    let lower = problems.to_lowercase();
    let text = if lower.contains("passphrase") || lower.contains("encrypt") {
        "the archive is protected by a password".to_string()
    } else if lower.contains("unrecognized archive format") || lower.contains("not supported") {
        "this kind of archive cannot be opened here".to_string()
    } else if lower.contains("truncated") || lower.contains("damaged") || lower.contains("corrupt")
    {
        "the archive is damaged or incomplete".to_string()
    } else {
        problems
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .map(|l| l.trim_start_matches("bsdtar: ").to_string())
            .unwrap_or_else(|| "the archive could not be extracted".to_string())
    };
    io::Error::new(io::ErrorKind::InvalidData, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_lose_their_archive_ending() {
        assert_eq!(folder_name(Path::new("/d/photos.zip")), "photos");
        assert_eq!(folder_name(Path::new("/d/site-1.2.tar.gz")), "site-1.2");
        assert_eq!(folder_name(Path::new("/d/Backup.TAR.XZ")), "Backup");
        assert_eq!(folder_name(Path::new("/d/music.rar")), "music");
        assert!(is_archive(Path::new("/d/a.7z")));
        assert!(!is_archive(Path::new("/d/notes.txt")));
        assert!(
            !is_archive(Path::new("/d/.zip")),
            "a bare ending is not an archive"
        );
    }

    #[test]
    fn complaints_become_sentences() {
        assert_eq!(
            explain("bsdtar: Passphrase required for this entry").to_string(),
            "the archive is protected by a password"
        );
        assert_eq!(
            explain("bsdtar: Error opening archive: Unrecognized archive format").to_string(),
            "this kind of archive cannot be opened here"
        );
        assert_eq!(explain("bsdtar: x: Disk full").to_string(), "x: Disk full");
    }
}
