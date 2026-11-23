//! The Markdown page (P7-T07; DESIGN.md §4.2, §8.5): opening a `.md` file
//! shows it with Preview, Edit and Split, an outline of its headings, and
//! Save and Revert. The preview is QML's own Markdown rendering (no web
//! engine, no scripts); remote images are removed before it sees the text.
//! Save goes through `core::ops`.
//!
//! Leaving the page with unsaved changes keeps them: opening the same file
//! again brings them back, until they are saved or reverted.

use std::path::{Path, PathBuf};

use gettextrs::gettext;
use serde_json::{Value, json};

use steward_core::markdown;

use super::{Controller, Event, dirty};
use crate::paths;

#[derive(Default, Debug)]
pub struct MarkdownState {
    path: Option<PathBuf>,
    /// The file as it is on disk (last loaded or saved).
    original: String,
    /// The text being edited.
    buffer: String,
    /// "preview", "edit" or "split".
    mode: String,
    /// Bumped whenever the editor must take the buffer (open, revert), so
    /// typing never resets the cursor.
    load: u64,
}

pub fn is_markdown(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(
            e.to_ascii_lowercase().as_str(),
            "md" | "markdown" | "mdown" | "mkd"
        )
    })
}

/// Larger files are opened in the default app instead.
const LIMIT: u64 = 4 * 1024 * 1024;

impl Controller {
    /// Opens a Markdown file on the page. Returns false when it cannot be
    /// read as text, so the caller opens it in another app.
    pub fn md_open(&mut self, path: &Path) -> bool {
        if std::fs::metadata(path)
            .map(|m| m.len() > LIMIT)
            .unwrap_or(true)
        {
            return false;
        }
        let same = self.md.path.as_deref() == Some(path);
        if !(same && self.md.buffer != self.md.original) {
            let Ok(text) = std::fs::read_to_string(path) else {
                return false;
            };
            self.md.original = text.clone();
            self.md.buffer = text;
            self.md.path = Some(path.to_path_buf());
        }
        if self.md.mode.is_empty() {
            self.md.mode = "preview".into();
        }
        self.md.load += 1;
        self.open_tool("markdown");
        true
    }

    pub fn md_mode(&mut self, mode: &str) {
        if matches!(mode, "preview" | "edit" | "split") {
            self.md.mode = mode.to_string();
            self.mark(dirty::TOOLS);
        }
    }

    pub fn md_edit(&mut self, text: &str) {
        if self.md.buffer != text {
            self.md.buffer = text.to_string();
            self.mark(dirty::TOOLS);
        }
    }

    pub fn md_save(&mut self) {
        let Some(path) = self.md.path.clone() else {
            return;
        };
        if self.tools.page != "markdown" || self.md.buffer == self.md.original {
            return;
        }
        match steward_core::ops::save_text(&path, &self.md.buffer) {
            Ok(()) => {
                self.md.original = self.md.buffer.clone();
                self.emit(Event::Toast(gettext("Saved"), false));
            }
            Err(error) => self.emit(Event::Toast(
                format!("{}: {error}", gettext("Could not save")),
                false,
            )),
        }
        self.mark(dirty::TOOLS);
    }

    /// Back to the file on disk (read again, in case it changed).
    pub fn md_revert(&mut self) {
        let Some(path) = self.md.path.clone() else {
            return;
        };
        if let Ok(text) = std::fs::read_to_string(&path) {
            self.md.original = text;
        }
        self.md.buffer = self.md.original.clone();
        self.md.load += 1;
        self.mark(dirty::TOOLS);
    }

    /// A link in the preview: web links open in the browser, links to other
    /// Markdown files in the same folder open here.
    pub fn md_link(&mut self, link: &str) {
        let folder = self
            .md
            .path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf);
        if !link.contains("://") {
            if let Some(folder) = folder {
                let target = folder.join(link.split('#').next().unwrap_or(link));
                if is_markdown(&target) && target.is_file() {
                    self.md_open(&target);
                    return;
                }
            }
            return;
        }
        let web = link.starts_with("http://")
            || link.starts_with("https://")
            || link.starts_with("mailto:");
        if !web {
            return;
        }
        if let Err(error) =
            gio::AppInfo::launch_default_for_uri(link, None::<&gio::AppLaunchContext>)
        {
            self.emit(Event::Toast(
                format!("{}: {error}", gettext("Could not open it")),
                false,
            ));
        }
    }

    pub fn md_name(&self) -> String {
        self.md
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    pub fn markdown_json(&self) -> Value {
        let Some(path) = &self.md.path else {
            return json!({ "name": "" });
        };
        let folder = path.parent().unwrap_or(Path::new("/"));
        let outline: Vec<Value> = markdown::outline(&self.md.buffer)
            .into_iter()
            .map(|h| json!({ "level": h.level, "text": h.text, "line": h.line }))
            .collect();
        json!({
            "name": self.md_name(),
            "path": path.display().to_string(),
            "where": paths::display_path(folder),
            "mode": self.md.mode,
            "load": self.md.load,
            "text": self.md.buffer,
            "lines": self.md.buffer.lines().count().max(1),
            "preview": markdown::preview(&self.md.buffer, folder),
            "base": format!("file://{}/", folder.display()),
            "outline": outline,
            "modified": self.md.buffer != self.md.original,
        })
    }

    #[cfg(feature = "e2e")]
    pub fn markdown_test_json(&self) -> Value {
        json!({
            "path": self.md.path.as_ref().map(|p| p.display().to_string()),
            "mode": self.md.mode,
            "modified": self.md.buffer != self.md.original,
            "buffer": self.md.buffer,
            "outline": markdown::outline(&self.md.buffer).into_iter().map(|h| h.text).collect::<Vec<_>>(),
            "preview": self.md.path.as_deref().and_then(Path::parent)
                .map(|f| markdown::preview(&self.md.buffer, f)),
        })
    }
}
