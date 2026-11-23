//! Smart Downloads (P4-T05, P4-T06): which folder is Downloads, and the
//! side panel's suggestions, worked out on a background thread.

use std::path::{Path, PathBuf};

use gettextrs::gettext;
use gio::prelude::*;
use serde_json::{Value, json};

use steward_core::index::Index;
use steward_core::ops::Request;
use steward_core::suggest::{self, Item, Suggestions};

use super::{Controller, dirty, later};
use crate::{format, settings};

#[derive(Default, Debug)]
pub struct DownloadsState {
    suggestions: Option<(PathBuf, Suggestions)>,
    computing: bool,
    ticket: u64,
}

/// The folder treated as Downloads: the preference, else the XDG folder.
pub fn downloads_folder() -> PathBuf {
    let chosen = settings::get().string("downloads-folder").to_string();
    if !chosen.is_empty() {
        return PathBuf::from(chosen);
    }
    glib::user_special_dir(glib::UserDirectory::Downloads)
        .unwrap_or_else(|| glib::home_dir().join("Downloads"))
}

fn names(items: &[Item]) -> Vec<String> {
    items
        .iter()
        .map(|i| {
            i.path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
        .collect()
}

impl Controller {
    fn in_downloads(&self) -> bool {
        self.current()
            .and_then(|f| f.path())
            .is_some_and(|p| p == downloads_folder())
    }

    /// Works the suggestions out again if the Downloads folder is on screen.
    pub fn refresh_suggestions(&mut self) {
        if !self.in_downloads() {
            return;
        }
        let folder = downloads_folder();
        self.downloads.ticket += 1;
        let ticket = self.downloads.ticket;
        self.downloads.computing = true;
        self.mark(dirty::PREVIEW);
        std::thread::spawn(move || {
            let index = Index::open_read_only(&Index::default_path()).ok();
            let cache =
                steward_core::dupes::HashCache::open(&steward_core::dupes::default_cache_path())
                    .or_else(|_| steward_core::dupes::HashCache::in_memory());
            let Ok(cache) = cache else { return };
            let found = suggest::downloads(
                &folder,
                steward_core::plan::now(),
                &suggest::Dpkg,
                index.as_ref(),
                &cache,
            );
            glib::MainContext::default().invoke(move || {
                later(move |c| {
                    if c.downloads.ticket == ticket {
                        c.downloads.computing = false;
                        c.downloads.suggestions = Some((folder, found));
                        c.mark(dirty::PREVIEW);
                    }
                })
            });
        });
    }

    /// "Move to Trash" on a suggestion card: one batch, with Undo.
    pub fn trash_suggestion(&mut self, id: &str) {
        let Some((_, found)) = &self.downloads.suggestions else {
            return;
        };
        let items = match id {
            "installers" => &found.installers,
            "duplicates" => &found.duplicates,
            "untouched" => &found.untouched,
            _ => return,
        };
        let sources: Vec<PathBuf> = items.iter().map(|i| i.path.clone()).collect();
        if sources.is_empty() {
            return;
        }
        let summary = format!(
            "{} {} {}",
            gettext("Move"),
            format::items(sources.len() as u32),
            gettext("to the Trash")
        );
        self.submit_request(Request::Trash { sources }, summary);
    }

    /// The panel's "Tidy Downloads…": the Tidy plan for the Downloads folder.
    pub fn tidy_downloads(&mut self) {
        let folder = downloads_folder();
        if self.current().and_then(|f| f.path()).as_deref() != Some(folder.as_path()) {
            self.navigate(&gio::File::for_path(&folder), true);
        }
        self.plan_tidy_in(&folder);
    }

    /// Preferences: choose the Downloads folder ("" for the system's own).
    pub fn set_downloads_folder(&mut self, text: &str) {
        let path = if text.trim().is_empty() {
            String::new()
        } else {
            crate::paths::expand(text.trim(), None)
                .display()
                .to_string()
        };
        if !path.is_empty() && !Path::new(&path).is_dir() {
            self.emit(super::Event::Toast(
                format!("{} “{}”", gettext("No folder called"), text.trim()),
                false,
            ));
            return;
        }
        let _ = settings::get().set_string("downloads-folder", &path);
    }

    /// Suggestion cards for the preview pane's overview (Downloads only).
    pub fn suggestions_json(&self) -> Value {
        if !self.in_downloads() {
            return Value::Null;
        }
        let Some((_, found)) = &self.downloads.suggestions else {
            return json!({ "computing": true, "cards": [] });
        };
        let mut cards = Vec::new();
        let mut card = |id: &str, items: &[Item], title: String, text: String| {
            if items.is_empty() {
                return;
            }
            let bytes: u64 = items.iter().map(|i| i.bytes).sum();
            cards.push(json!({
                "id": id, "title": format!("{title} · {}", format::size(bytes)), "text": text,
                "names": names(items),
            }));
        };
        card(
            "installers",
            &found.installers,
            format!(
                "{} {}",
                found.installers.len(),
                if found.installers.len() == 1 {
                    gettext("installer")
                } else {
                    gettext("installers")
                }
            ),
            found
                .installers
                .first()
                .map(|i| format!("{} {}", i.why, gettext("already")))
                .unwrap_or_default(),
        );
        card(
            "duplicates",
            &found.duplicates,
            format!(
                "{} {}",
                found.duplicates.len(),
                gettext("likely duplicates")
            ),
            found
                .duplicates
                .first()
                .map(|i| i.why.clone())
                .unwrap_or_default(),
        );
        card(
            "untouched",
            &found.untouched,
            format!(
                "{} {}",
                found.untouched.len(),
                gettext("untouched for 90 days")
            ),
            if found.atime_trusted {
                gettext("Not opened in 90 days")
            } else {
                gettext("Not changed in 90 days")
            },
        );
        json!({ "computing": self.downloads.computing, "cards": cards })
    }

    #[cfg(feature = "e2e")]
    pub fn downloads_test_json(&self) -> Value {
        json!({
            "folder": downloads_folder().display().to_string(),
            "here": self.in_downloads(),
            "computing": self.downloads.computing,
            "installers": self.downloads.suggestions.as_ref().map(|(_, s)| names(&s.installers)),
            "duplicates": self.downloads.suggestions.as_ref().map(|(_, s)| names(&s.duplicates)),
            "untouched": self.downloads.suggestions.as_ref().map(|(_, s)| names(&s.untouched)),
        })
    }
}
