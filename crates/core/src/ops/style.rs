//! Folder symbols and colours chosen by the user (P4-T10). Stored in
//! extended attributes (`user.cairn.symbol`, `user.cairn.color`), which
//! travel with the folder; where the filesystem has none (or the test
//! switch CAIRN_NO_XATTR is set), in app.db keyed by filesystem and inode.
//! Either way the choice survives a rename.

use std::os::unix::fs::MetadataExt;
use std::path::Path;

use rusqlite::{OptionalExtension, params};

use crate::store::Store;

const SYMBOL: &str = "user.cairn.symbol";
const COLOR: &str = "user.cairn.color";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Style {
    pub symbol: Option<String>,
    pub color: Option<String>,
}

impl Style {
    pub fn is_empty(&self) -> bool {
        self.symbol.is_none() && self.color.is_none()
    }
}

/// False when the test switch turns extended attributes off.
pub fn xattrs_enabled() -> bool {
    std::env::var_os("CAIRN_NO_XATTR").is_none()
}

fn get_xattr(path: &Path, name: &str) -> Option<String> {
    let mut buffer = [0u8; 256];
    let n = rustix::fs::getxattr(path, name, &mut buffer).ok()?;
    let text = String::from_utf8_lossy(&buffer[..n]).into_owned();
    (!text.is_empty()).then_some(text)
}

fn set_xattr(path: &Path, name: &str, value: Option<&str>) -> std::io::Result<()> {
    match value {
        Some(v) => rustix::fs::setxattr(path, name, v.as_bytes(), rustix::fs::XattrFlags::empty())
            .map_err(Into::into),
        None => match rustix::fs::removexattr(path, name) {
            Ok(()) | Err(rustix::io::Errno::NODATA) => Ok(()),
            Err(e) => Err(e.into()),
        },
    }
}

/// The chosen style of `path`, if any.
pub fn read(path: &Path, store: &Store) -> Style {
    read_in(path, store, xattrs_enabled())
}

/// As [`read`], with extended attributes used or not.
pub fn read_in(path: &Path, store: &Store, attrs: bool) -> Style {
    if attrs {
        let style = Style {
            symbol: get_xattr(path, SYMBOL),
            color: get_xattr(path, COLOR),
        };
        if !style.is_empty() {
            return style;
        }
    }
    let Ok(meta) = path.metadata() else {
        return Style::default();
    };
    store
        .conn()
        .query_row(
            "SELECT symbol, color FROM folder_style WHERE dev = ?1 AND inode = ?2",
            params![meta.dev() as i64, meta.ino() as i64],
            |r| {
                Ok(Style {
                    symbol: r.get(0)?,
                    color: r.get(1)?,
                })
            },
        )
        .optional()
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// Sets (or, with an empty style, clears) the chosen style of `path`.
pub fn write(path: &Path, style: &Style, store: &Store) -> std::io::Result<()> {
    write_in(path, style, store, xattrs_enabled())
}

/// As [`write`], with extended attributes used or not.
pub fn write_in(path: &Path, style: &Style, store: &Store, attrs: bool) -> std::io::Result<()> {
    let meta = path.metadata()?;
    let in_attrs = attrs
        && set_xattr(path, SYMBOL, style.symbol.as_deref()).is_ok()
        && set_xattr(path, COLOR, style.color.as_deref()).is_ok();
    // The database keeps it when attributes cannot (and forgets an old
    // fallback when they can).
    let conn = store.conn();
    conn.execute(
        "DELETE FROM folder_style WHERE dev = ?1 AND inode = ?2",
        params![meta.dev() as i64, meta.ino() as i64],
    )
    .map_err(std::io::Error::other)?;
    if !in_attrs && !style.is_empty() {
        conn.execute(
            "INSERT INTO folder_style (dev, inode, path, symbol, color) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![meta.dev() as i64, meta.ino() as i64, path.to_string_lossy(), style.symbol, style.color],
        )
        .map_err(std::io::Error::other)?;
    }
    Ok(())
}
