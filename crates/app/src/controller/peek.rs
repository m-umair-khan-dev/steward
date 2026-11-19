//! Peek (Space, P3-T11): a large preview of one file over the window, with
//! its facts. The arrows move through the folder (or the search results).

use std::io::Read;
use std::path::{Path, PathBuf};

use gettextrs::gettext;
use gio::prelude::*;
use serde_json::{Value, json};

use cairn_core::Kind;

use super::{Controller, Event, dirty};
use crate::{format, paths, tint};

/// Text shown for text and code files.
const TEXT_LIMIT: usize = 64 * 1024;

#[derive(Default, Debug)]
pub struct PeekState {
    pub open: bool,
    list: Vec<PathBuf>,
    at: usize,
    /// Pages of the PDF on show, as QML reports them.
    pages: Option<u32>,
}

fn mode_of(path: &Path) -> (&'static str, String) {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (content_type, _) = gio::content_type_guess(Some(name.as_str()), None);
    let kind = Kind::from_content_type(&content_type);
    let mode = if path.is_dir() {
        "folder"
    } else if content_type.starts_with("image/") {
        "image"
    } else if content_type == "application/pdf" {
        "pdf"
    } else if content_type.starts_with("video/") || content_type.starts_with("audio/") {
        "media"
    } else if gio::content_type_is_a(&content_type, "text/plain") || kind == Kind::Code {
        "text"
    } else {
        "other"
    };
    (mode, content_type.to_string())
}

/// The start of a text file, or None if it does not look like text.
fn read_text(path: &Path) -> Option<String> {
    let mut bytes = Vec::with_capacity(TEXT_LIMIT);
    std::fs::File::open(path)
        .ok()?
        .take(TEXT_LIMIT as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.contains(&0) {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

impl Controller {
    /// Space in the folder: Peek at the selected item, then move through
    /// what the folder shows.
    pub fn peek_selected(&mut self) {
        let list: Vec<PathBuf> = self
            .visible
            .iter()
            .filter_map(|e| e.data().file.path())
            .collect();
        let Some(at) = self.anchor.filter(|&a| a < list.len()).or_else(|| {
            let selected = self.selected();
            let first = selected.first()?.data().file.path()?;
            list.iter().position(|p| *p == first)
        }) else {
            return;
        };
        self.peek_list(list, at);
    }

    /// Peek at one path (from search); the arrows move through the results.
    pub fn peek_path(&mut self, path: &Path) {
        let list: Vec<PathBuf> = self.search_paths();
        let at = list.iter().position(|p| p == path).unwrap_or(0);
        if list.is_empty() {
            self.peek_list(vec![path.to_path_buf()], 0);
        } else {
            self.peek_list(list, at);
        }
    }

    fn peek_list(&mut self, list: Vec<PathBuf>, at: usize) {
        self.peek = PeekState {
            open: true,
            list,
            at,
            pages: None,
        };
        self.mark(dirty::PEEK);
        self.emit(Event::Focus("peek"));
    }

    pub fn peek_move(&mut self, direction: &str) {
        if !self.peek.open || self.peek.list.is_empty() {
            return;
        }
        let last = self.peek.list.len() - 1;
        self.peek.at = match direction {
            "next" => (self.peek.at + 1).min(last),
            _ => self.peek.at.saturating_sub(1),
        };
        // Keep the folder's selection in step with what is on show.
        if !self.search.open
            && let Some(path) = self.peek.list.get(self.peek.at).cloned()
            && let Some(pos) = self
                .visible
                .iter()
                .position(|e| e.data().file.path().as_deref() == Some(&path))
        {
            self.pick(pos, 0);
        }
        self.peek.pages = None;
        self.mark(dirty::PEEK);
    }

    pub fn peek_close(&mut self) {
        if self.peek.open {
            self.peek.open = false;
            self.mark(dirty::PEEK);
            self.emit(Event::Focus(if self.search.open {
                "search"
            } else {
                "content"
            }));
        }
    }

    /// Opens the file on show in its app.
    pub fn peek_open(&mut self) {
        let Some(path) = self.peek.list.get(self.peek.at).cloned() else {
            return;
        };
        self.peek_close();
        let uri = gio::File::for_path(&path).uri().to_string();
        glib::spawn_future_local(async move {
            if let Err(error) =
                gio::AppInfo::launch_default_for_uri_future(&uri, gio::AppLaunchContext::NONE).await
            {
                glib::g_warning!("cairn", "could not open {uri}: {error}");
            }
        });
    }

    pub fn peek_pages(&mut self, pages: u32) {
        self.peek.pages = Some(pages);
        self.mark(dirty::PEEK);
    }

    pub fn peek_json(&self) -> Value {
        let Some(path) = self.peek.list.get(self.peek.at).filter(|_| self.peek.open) else {
            return json!({ "open": false });
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (mode, content_type) = mode_of(path);
        let kind = Kind::from_content_type(&content_type);
        let meta = std::fs::metadata(path).ok();
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let modified = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let mut facts = vec![
            json!([
                gettext("Kind"),
                gio::content_type_get_description(&content_type).to_string()
            ]),
            json!([gettext("Size"), format::size(size)]),
            json!([gettext("Modified"), format::date(modified)]),
        ];
        if let Some(pages) = self.peek.pages {
            facts.push(json!([gettext("Pages"), pages.to_string()]));
        }
        if let Some(parent) = path.parent() {
            facts.push(json!([gettext("Where"), paths::display_path(parent)]));
        }
        json!({
            "open": true,
            "name": name,
            "uri": gio::File::for_path(path).uri().to_string(),
            "mode": mode,
            "kind": kind.id(),
            "badge": tint::badge(&name, kind),
            "meta": format!("{} · {} · {}", tint::badge(&name, kind), format::size(size), format::date(modified)),
            "text": if mode == "text" { read_text(path) } else { None },
            "facts": facts,
            "position": format!("{} {} {}", self.peek.at + 1, gettext("of"), self.peek.list.len()),
            "pages": self.peek.pages,
        })
    }

    #[cfg(feature = "e2e")]
    pub fn peek_test_json(&self) -> Value {
        let full = self.peek_json();
        json!({
            "open": full["open"], "name": full["name"], "mode": full["mode"], "pages": full["pages"],
            "has_text": full["text"].is_string(),
        })
    }
}
