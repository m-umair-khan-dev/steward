//! Folder symbols and colours (P4-T10): automatic rules, and the user's
//! own choice (extended attributes, with an app.db fallback).

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

use gettextrs::gettext;
use gio::prelude::*;
use serde_json::json;

use steward_core::Kind;
use steward_core::ops::style::{self, Style};

use super::{Controller, dirty};
use crate::app_data;

/// Symbols the chooser offers (our own icons).
pub const SYMBOLS: [&str; 16] = [
    "document",
    "receipt",
    "coin",
    "plane",
    "briefcase",
    "box",
    "pictures",
    "code",
    "pdf",
    "music",
    "videos",
    "star",
    "lock",
    "package",
    "downloads",
    "desktop",
];
/// Colours the chooser offers (kind tints).
pub const COLORS: [&str; 9] = [
    "folders",
    "documents",
    "images",
    "videos",
    "audio",
    "pdfs",
    "archives",
    "code",
    "installers",
];

/// Projects of these kinds get the code symbol.
const PROJECT_MARKERS: [&str; 6] = [
    ".git",
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "go.mod",
    "Makefile",
];

#[derive(Default, Debug)]
pub struct StyleState {
    /// (symbol, colour) per folder URI, worked out once per listing.
    cache: RefCell<HashMap<String, (String, String)>>,
    /// The folder the chooser is open for.
    choosing: Option<std::path::PathBuf>,
}

fn kind_symbol(kind: Kind) -> &'static str {
    match kind {
        Kind::Images => "pictures",
        Kind::Videos => "videos",
        Kind::Audio => "music",
        Kind::Pdfs => "pdf",
        Kind::Archives => "box",
        Kind::Installers => "package",
        Kind::Code => "code",
        _ => "document",
    }
}

/// The automatic symbol of a folder (DESIGN.md §4.1): special folders, then
/// projects (Git or a build file), then words in its name, then the kind of
/// most files inside.
pub fn auto_symbol(path: &Path, name: &str) -> &'static str {
    let home = glib::home_dir();
    let special = [
        (glib::UserDirectory::Desktop, "desktop"),
        (glib::UserDirectory::Documents, "documents"),
        (glib::UserDirectory::Downloads, "downloads"),
        (glib::UserDirectory::Music, "music"),
        (glib::UserDirectory::Pictures, "pictures"),
        (glib::UserDirectory::Videos, "videos"),
    ];
    for (dir, icon) in special {
        if glib::user_special_dir(dir).is_some_and(|p| p == path && p != home) {
            return icon;
        }
    }
    if PROJECT_MARKERS.iter().any(|m| path.join(m).exists()) {
        return "code";
    }
    let lower = name.to_lowercase();
    let by_name = match lower.as_str() {
        n if n.contains("invoice") || n.contains("receipt") || n.contains("bill") => {
            Some("receipt")
        }
        n if n.contains("tax") || n.contains("budget") => Some("coin"),
        n if n.contains("travel") || n.contains("trip") => Some("plane"),
        n if n.contains("client") || n.contains("work") => Some("briefcase"),
        n if n.contains("archive") || n.contains("backup") => Some("box"),
        n if n.contains("wallpaper") || n.contains("photo") || n.contains("picture") => {
            Some("pictures")
        }
        n if n.contains("project") || n.contains("code") => Some("code"),
        _ => None,
    };
    if let Some(symbol) = by_name {
        return symbol;
    }
    // Mostly one kind inside (at least 60 % of the files).
    let mut counts: HashMap<Kind, usize> = HashMap::new();
    let mut total = 0usize;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten().take(300) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || entry.path().is_dir() {
                continue;
            }
            total += 1;
            *counts
                .entry(steward_core::plan::kind_of(&name))
                .or_default() += 1;
        }
    }
    if let Some((kind, n)) = counts.into_iter().max_by_key(|(_, n)| *n)
        && total > 0
        && n * 10 >= total * 6
    {
        return kind_symbol(kind);
    }
    "document"
}

impl Controller {
    /// (symbol, colour) of a folder: the user's choice, else the rules.
    pub fn folder_look(&self, path: &Path, uri: &str, name: &str) -> (String, String) {
        if let Some(found) = self.style.cache.borrow().get(uri) {
            return found.clone();
        }
        let chosen = style::read(path, &app_data::store().borrow());
        let look = (
            chosen
                .symbol
                .unwrap_or_else(|| auto_symbol(path, name).to_string()),
            chosen.color.unwrap_or_else(|| "folders".into()),
        );
        self.style
            .cache
            .borrow_mut()
            .insert(uri.to_string(), look.clone());
        look
    }

    pub fn forget_looks(&self) {
        self.style.cache.borrow_mut().clear();
    }

    /// "Symbol and Colour…" for the selected folder.
    pub fn ask_folder_style(&mut self) {
        let selected = self.selected();
        let [entry] = selected.as_slice() else { return };
        if !entry.is_dir() {
            return;
        }
        let Some(path) = entry.data().file.path() else {
            return;
        };
        let (symbol, color) = self.folder_look(&path, &entry.data().uri(), entry.name());
        self.dialog_data = json!({
            "name": entry.data().display_name, "symbol": symbol, "color": color,
            "symbols": SYMBOLS, "colors": COLORS,
        });
        self.style.choosing = Some(path);
        self.show_dialog("folder-style");
    }

    /// "symbol|colour", or "auto" to go back to the rules.
    pub fn set_folder_style(&mut self, arg: &str) {
        let Some(path) = self.style.choosing.take() else {
            return;
        };
        let chosen = if arg == "auto" {
            Style::default()
        } else {
            let (symbol, color) = arg.split_once('|').unwrap_or((arg, ""));
            Style {
                symbol: (!symbol.is_empty()).then(|| symbol.to_string()),
                color: (!color.is_empty()).then(|| color.to_string()),
            }
        };
        if let Err(error) = style::write(&path, &chosen, &app_data::store().borrow()) {
            self.emit(super::Event::Toast(
                format!("{} {error}", gettext("Could not change the folder:")),
                false,
            ));
        }
        if self.dialog == "folder-style" {
            self.close_dialog();
        }
        self.forget_looks();
        self.mark(dirty::ROWS | dirty::ITEMS | dirty::PREVIEW);
    }

    #[cfg(feature = "e2e")]
    pub fn styles_test_json(&self) -> serde_json::Value {
        serde_json::Value::Object(
            self.visible
                .iter()
                .filter(|e| e.is_dir())
                .filter_map(|e| {
                    let path = e.data().file.path()?;
                    let (symbol, color) = self.folder_look(&path, &e.data().uri(), e.name());
                    Some((
                        e.name().to_string(),
                        json!({ "symbol": symbol, "color": color }),
                    ))
                })
                .collect(),
        )
    }
}
