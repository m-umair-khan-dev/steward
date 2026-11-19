//! The FreeDesktop Trash, implemented directly so that every trashed item's
//! exact place is known (restore is then a rename). Items stay compatible
//! with Files (Nautilus) and GIO's `trash:///`.
//!
//! Files on the home filesystem go to `$XDG_DATA_HOME/Trash`; files on other
//! filesystems go to `$topdir/.Trash-$uid` on that filesystem.

use std::io::{self, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use super::fsx;

/// One item in a Trash folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashItem {
    /// The item itself, in `…/Trash/files/`.
    pub path: PathBuf,
    /// Where it was before.
    pub original: PathBuf,
    /// "YYYY-MM-DDThh:mm:ss", local time, as written in the info file.
    pub deleted: String,
    pub is_dir: bool,
    pub size: u64,
}

/// Where the Trash folders are. Tests point `home` inside their own folder.
#[derive(Debug, Clone)]
pub struct Trash {
    pub home: PathBuf,
}

impl Trash {
    pub fn new(home: PathBuf) -> Self {
        Self { home }
    }

    /// `$XDG_DATA_HOME/Trash`.
    pub fn default_home() -> PathBuf {
        glib::user_data_dir().join("Trash")
    }

    /// The Trash folder that takes `path` without crossing filesystems, and
    /// the folder its info paths are relative to (None: absolute paths).
    fn root_for(&self, path: &Path) -> io::Result<(PathBuf, Option<PathBuf>)> {
        if fsx::same_device(path, &self.home) {
            return Ok((self.home.clone(), None));
        }
        let top = top_dir(path).ok_or_else(|| io::Error::other("no mount point"))?;
        let uid = rustix::process::getuid().as_raw();
        let root = top.join(format!(".Trash-{uid}"));
        if !root.exists() {
            std::fs::create_dir(&root)?;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok((root, Some(top)))
    }

    /// Creates the home Trash folders if they are missing (the Trash view
    /// lists `files/`).
    pub fn ensure(&self) -> io::Result<PathBuf> {
        std::fs::create_dir_all(self.home.join("info"))?;
        let files = self.home.join("files");
        std::fs::create_dir_all(&files)?;
        Ok(files)
    }

    /// Moves `path` to the Trash and returns where it now is.
    pub fn trash(&self, path: &Path) -> io::Result<PathBuf> {
        if !fsx::exists(path) {
            return Err(io::Error::from(io::ErrorKind::NotFound));
        }
        let (root, relative_to) = self.root_for(path)?;
        let files = root.join("files");
        let info = root.join("info");
        std::fs::create_dir_all(&files)?;
        std::fs::create_dir_all(&info)?;
        let base = fsx::file_name(path);
        let recorded = match &relative_to {
            Some(top) => path.strip_prefix(top).unwrap_or(path).to_path_buf(),
            None => path.to_path_buf(),
        };
        let date = glib::DateTime::now_local()
            .and_then(|d| d.format("%Y-%m-%dT%H:%M:%S"))
            .map(|s| s.to_string())
            .unwrap_or_default();
        for n in 1.. {
            let name = if n == 1 {
                base.clone()
            } else {
                format!("{base}.{n}")
            };
            let info_path = info.join(format!("{name}.trashinfo"));
            // The info file is created first and exclusively: it reserves the name.
            let mut file = match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&info_path)
            {
                Ok(file) => file,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            };
            if fsx::exists(&files.join(&name)) {
                drop(file);
                let _ = std::fs::remove_file(&info_path);
                continue;
            }
            write!(
                file,
                "[Trash Info]\nPath={}\nDeletionDate={date}\n",
                escape(&recorded)
            )?;
            file.sync_all()?;
            let target = files.join(&name);
            if let Err(e) = fsx::rename_noreplace(path, &target) {
                let _ = std::fs::remove_file(&info_path);
                return Err(e);
            }
            return Ok(target);
        }
        unreachable!()
    }

    /// Puts a trashed item back where it was (never replacing anything).
    pub fn restore(&self, item: &Path, to: &Path) -> io::Result<()> {
        if let Some(parent) = to.parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent)?;
        }
        fsx::rename_noreplace(item, to)?;
        let _ = std::fs::remove_file(info_path(item));
        Ok(())
    }

    /// Deletes a trashed item for good.
    pub fn erase(&self, item: &Path) -> io::Result<()> {
        if fsx::exists(item) {
            fsx::remove_tree(item)?;
        }
        let _ = std::fs::remove_file(info_path(item));
        Ok(())
    }

    /// Items in the home Trash, newest first.
    pub fn list(&self) -> Vec<TrashItem> {
        let mut items = list_root(&self.home, None);
        items.sort_by(|a, b| b.deleted.cmp(&a.deleted).then_with(|| a.path.cmp(&b.path)));
        items
    }

    /// Reads the original place of a trashed item from its info file.
    pub fn original_of(&self, item: &Path) -> Option<PathBuf> {
        let root = item.parent()?.parent()?;
        let top = root
            .file_name()
            .filter(|n| n.to_string_lossy().starts_with(".Trash-"))
            .and_then(|_| root.parent());
        parse_info(&info_path(item), top).map(|(path, _)| path)
    }
}

fn info_path(item: &Path) -> PathBuf {
    let root = item
        .parent()
        .and_then(Path::parent)
        .unwrap_or(Path::new("/"));
    root.join("info")
        .join(format!("{}.trashinfo", fsx::file_name(item)))
}

fn list_root(root: &Path, top: Option<&Path>) -> Vec<TrashItem> {
    let Ok(entries) = std::fs::read_dir(root.join("files")) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let (original, deleted) = parse_info(&info_path(&path), top)?;
            let meta = path.symlink_metadata().ok()?;
            Some(TrashItem {
                is_dir: meta.is_dir(),
                size: if meta.is_dir() {
                    fsx::measure(&path).0
                } else {
                    meta.len()
                },
                path,
                original,
                deleted,
            })
        })
        .collect()
}

fn parse_info(path: &Path, top: Option<&Path>) -> Option<(PathBuf, String)> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut original = None;
    let mut deleted = String::new();
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("Path=") {
            let p = PathBuf::from(std::ffi::OsString::from_vec(unescape(value)?));
            original = Some(match top {
                Some(top) if p.is_relative() => top.join(p),
                _ => p,
            });
        } else if let Some(value) = line.strip_prefix("DeletionDate=") {
            deleted = value.to_string();
        }
    }
    Some((original?, deleted))
}

/// Percent-encodes a path as the spec asks (like a URL path).
fn escape(path: &Path) -> String {
    let mut out = String::new();
    for &byte in path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"/-_.~!$&'()*+,;=:@".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn unescape(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(out)
}

/// The mount point that holds `path`: the last ancestor on the same device.
fn top_dir(path: &Path) -> Option<PathBuf> {
    let dev = fsx::device_of(path)?;
    let mut top = None;
    for ancestor in path.ancestors().skip(1) {
        match ancestor.symlink_metadata() {
            Ok(meta) if meta.dev() == dev => top = Some(ancestor.to_path_buf()),
            Ok(_) => break,
            Err(_) => continue,
        }
    }
    top
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_round_trip_through_the_info_encoding() {
        let path = Path::new("/home/sam/Q3 report (final) 100%.pdf");
        let escaped = escape(path);
        assert_eq!(escaped, "/home/sam/Q3%20report%20(final)%20100%25.pdf");
        assert_eq!(unescape(&escaped).unwrap(), path.as_os_str().as_bytes());
        let odd = Path::new(std::ffi::OsStr::from_bytes(b"/tmp/caf\xc3\xa9\n\xff"));
        assert_eq!(unescape(&escape(odd)).unwrap(), odd.as_os_str().as_bytes());
    }
}
