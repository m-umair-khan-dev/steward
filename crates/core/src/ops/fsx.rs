//! Filesystem primitives for the operations engine. Nothing here overwrites:
//! renames use `RENAME_NOREPLACE`, copies and new folders fail if the target
//! exists. Only `ops` may call these (AGENTS.md rule 10).

use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use gio::prelude::*;
use rustix::fs::{CWD, RenameFlags};
use rustix::io::Errno;

/// Renames without ever replacing `to`. Fails with `AlreadyExists` if `to`
/// exists and with `CrossesDevices` (EXDEV) across filesystems.
pub fn rename_noreplace(from: &Path, to: &Path) -> io::Result<()> {
    match rustix::fs::renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE) {
        Ok(()) => Ok(()),
        // Filesystems without RENAME_NOREPLACE: check, then rename. A narrow
        // race remains there; on ext4, btrfs, xfs and tmpfs the kernel decides.
        Err(Errno::INVAL) | Err(Errno::NOSYS) => {
            if exists(to) {
                return Err(io::Error::from(io::ErrorKind::AlreadyExists));
            }
            std::fs::rename(from, to)
        }
        Err(errno) => Err(io::Error::from(errno)),
    }
}

/// True if anything (file, folder, link, even a dangling one) has this path.
pub fn exists(path: &Path) -> bool {
    path.symlink_metadata().is_ok()
}

pub fn same_device(a: &Path, b: &Path) -> bool {
    match (device_of(a), device_of(b)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// The device of `path`, or of its nearest existing ancestor.
pub fn device_of(path: &Path) -> Option<u64> {
    path.ancestors()
        .find_map(|p| p.symlink_metadata().ok())
        .map(|m| m.dev())
}

/// Bytes and items under `path` (links count as themselves, not their target).
pub fn measure(path: &Path) -> (u64, u64) {
    let Ok(meta) = path.symlink_metadata() else {
        return (0, 0);
    };
    if !meta.is_dir() {
        return (meta.len(), 1);
    }
    let mut total = (0, 1);
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let (bytes, items) = measure(&entry.path());
            total.0 += bytes;
            total.1 += items;
        }
    }
    total
}

/// What went wrong in a copy, and whether `dst` was created by us.
pub struct CopyError {
    pub error: io::Error,
    /// True once `dst` exists because of this copy; the caller removes it.
    pub created: bool,
}

/// Copies `src` to `dst` (which must not exist): files with their metadata,
/// folders recursively, symbolic links as links. `progress` gets byte deltas.
pub fn copy_tree(
    src: &Path,
    dst: &Path,
    cancel: &gio::Cancellable,
    progress: &mut dyn FnMut(u64),
) -> Result<(), CopyError> {
    let meta = src.symlink_metadata().map_err(|error| CopyError {
        error,
        created: false,
    })?;
    if exists(dst) {
        return Err(CopyError {
            error: io::Error::from(io::ErrorKind::AlreadyExists),
            created: false,
        });
    }
    if meta.file_type().is_symlink() {
        let target = std::fs::read_link(src).map_err(|error| CopyError {
            error,
            created: false,
        })?;
        std::os::unix::fs::symlink(target, dst).map_err(|error| CopyError {
            error,
            created: false,
        })?;
        progress(meta.len());
        return Ok(());
    }
    if meta.is_dir() {
        std::fs::create_dir(dst).map_err(|error| CopyError {
            error,
            created: false,
        })?;
        let mut inner = || -> io::Result<()> {
            for entry in std::fs::read_dir(src)? {
                if cancel.is_cancelled() {
                    return Err(cancelled());
                }
                let entry = entry?;
                copy_tree(
                    &entry.path(),
                    &dst.join(entry.file_name()),
                    cancel,
                    progress,
                )
                .map_err(|e| e.error)?;
            }
            std::fs::set_permissions(dst, meta.permissions())?;
            Ok(())
        };
        return inner().map_err(|error| CopyError {
            error,
            created: true,
        });
    }
    copy_file(src, dst, &meta, cancel, progress)
}

/// Copies one regular file: in 1 MiB blocks with progress after each and a
/// cancel check before each; all-zero blocks become holes (like
/// `cp --sparse=auto`), so a sparse disk image stays small. Permissions and
/// times are kept.
fn copy_file(
    src: &Path,
    dst: &Path,
    meta: &std::fs::Metadata,
    cancel: &gio::Cancellable,
    progress: &mut dyn FnMut(u64),
) -> Result<(), CopyError> {
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::os::unix::fs::OpenOptionsExt;

    let mut input = std::fs::File::open(src).map_err(|error| CopyError {
        error,
        created: false,
    })?;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(dst)
        .map_err(|error| CopyError {
            error,
            created: false,
        })?;
    let fail = |error: io::Error| CopyError {
        error,
        created: true,
    };
    let mut buffer = vec![0u8; 1 << 20];
    let mut written: u64 = 0;
    loop {
        if cancel.is_cancelled() {
            return Err(fail(cancelled()));
        }
        let n = input.read(&mut buffer).map_err(fail)?;
        if n == 0 {
            break;
        }
        let block = &buffer[..n];
        if block.iter().all(|&b| b == 0) {
            output.seek(SeekFrom::Current(n as i64)).map_err(fail)?;
        } else {
            output.write_all(block).map_err(fail)?;
        }
        written += n as u64;
        progress(n as u64);
    }
    // A trailing hole is only a seek so far: fix the length.
    output.set_len(written).map_err(fail)?;
    output.set_permissions(meta.permissions()).map_err(fail)?;
    let mut times = std::fs::FileTimes::new();
    if let Ok(modified) = meta.modified() {
        times = times.set_modified(modified);
    }
    if let Ok(accessed) = meta.accessed() {
        times = times.set_accessed(accessed);
    }
    output.set_times(times).map_err(fail)?;
    output.sync_all().map_err(fail)?;
    drop(output);
    // Extended attributes (tags, origin URLs), where the filesystem allows.
    // Best effort: owner and the like need privileges we never have.
    let _ = gio::File::for_path(src).copy_attributes(
        &gio::File::for_path(dst),
        gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
        gio::Cancellable::NONE,
    );
    if written != meta.len() {
        return Err(fail(io::Error::other(
            "the file changed while it was copied",
        )));
    }
    Ok(())
}

/// Removes a file, link or whole folder (links are removed, never followed).
pub fn remove_tree(path: &Path) -> io::Result<()> {
    let meta = path.symlink_metadata()?;
    if meta.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

pub fn remove_empty_dir(path: &Path) -> io::Result<()> {
    std::fs::remove_dir(path)
}

pub fn create_dir(path: &Path) -> io::Result<()> {
    std::fs::create_dir(path)
}

/// Moves `src` to `dst` without replacing anything: a rename on the same
/// filesystem, otherwise a copy followed by removing the source.
pub fn move_path(
    src: &Path,
    dst: &Path,
    cancel: &gio::Cancellable,
    progress: &mut dyn FnMut(u64),
) -> io::Result<()> {
    match rename_noreplace(src, dst) {
        Ok(()) => {
            progress(measure(dst).0);
            Ok(())
        }
        Err(error) if error.raw_os_error() == Some(Errno::XDEV.raw_os_error()) => {
            if let Err(failure) = copy_tree(src, dst, cancel, progress) {
                if failure.created {
                    let _ = remove_tree(dst);
                }
                return Err(failure.error);
            }
            remove_tree(src)
        }
        Err(error) => Err(error),
    }
}

pub fn cancelled() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "cancelled")
}

pub fn is_cancelled(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::Interrupted
}

/// A name in `dir` that is free, starting from `name`: "a.txt", then
/// "a (2).txt", "a (3).txt" … Folders and dotfiles keep their whole name.
pub fn free_name(dir: &Path, name: &str) -> String {
    if !exists(&dir.join(name)) {
        return name.to_string();
    }
    let (stem, ext) = split_extension(dir, name);
    (2..)
        .map(|n| format!("{stem} ({n}){ext}"))
        .find(|candidate| !exists(&dir.join(candidate)))
        .expect("some name is free")
}

fn split_extension<'a>(dir: &Path, name: &'a str) -> (&'a str, &'a str) {
    if dir.join(name).is_dir() {
        return (name, "");
    }
    // Keep double extensions such as .tar.gz together.
    for double in [".tar.gz", ".tar.xz", ".tar.bz2", ".tar.zst"] {
        if let Some(stem) = name.strip_suffix(double).filter(|s| !s.is_empty()) {
            return (stem, &name[stem.len()..]);
        }
    }
    match name.rfind('.') {
        Some(0) | None => (name, ""),
        Some(dot) => (&name[..dot], &name[dot..]),
    }
}

pub fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Joins `dir` and a name, refusing names that would escape `dir`.
pub fn child(dir: &Path, name: &str) -> io::Result<PathBuf> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\0') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a valid name",
        ));
    }
    Ok(dir.join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_names_follow_the_pattern() {
        let dir = std::env::temp_dir().join(format!("steward-fsx-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(free_name(&dir, "a.txt"), "a.txt");
        std::fs::write(dir.join("a.txt"), "").unwrap();
        assert_eq!(free_name(&dir, "a.txt"), "a (2).txt");
        std::fs::write(dir.join("a (2).txt"), "").unwrap();
        assert_eq!(free_name(&dir, "a.txt"), "a (3).txt");
        std::fs::write(dir.join("b.tar.gz"), "").unwrap();
        assert_eq!(free_name(&dir, "b.tar.gz"), "b (2).tar.gz");
        std::fs::write(dir.join(".hidden"), "").unwrap();
        assert_eq!(free_name(&dir, ".hidden"), ".hidden (2)");
        std::fs::create_dir(dir.join("v1.0")).unwrap();
        assert_eq!(free_name(&dir, "v1.0"), "v1.0 (2)");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn names_cannot_escape_the_folder() {
        let dir = Path::new("/tmp");
        assert!(child(dir, "../x").is_err());
        assert!(child(dir, "..").is_err());
        assert!(child(dir, "").is_err());
        assert_eq!(child(dir, "ok").unwrap(), Path::new("/tmp/ok"));
    }
}
