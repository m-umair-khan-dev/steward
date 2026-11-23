//! The Shelf (P8-T01; DESIGN.md §4.4): a small window summoned from
//! anywhere (`app.shelf`, `steward shelf`, the tray icon, the global key).
//! Numbered pinned folders (keys 1 to 9) with item counts, Holding (files
//! remembered without moving them, kept across restarts), the latest
//! downloads, quick access, and a result line with Undo for drops.
//!
//! A Wayland app cannot choose where its window goes, so the compositor
//! places the Shelf; the optional Shell extension (P8-T05) may pin it.

use std::path::{Path, PathBuf};

use gettextrs::gettext;
use gio::prelude::*;
use serde_json::{Value, json};

use steward_core::ops::{JobId, Request};
use steward_core::shelf;

use super::{Controller, dirty, downloads};
use crate::{app_data, format, paths, tint};

#[derive(Default, Debug)]
pub struct ShelfState {
    pub open: bool,
    /// The last drop's outcome, and whether Undo applies.
    result: Option<(String, bool)>,
    /// The operation the Shelf started, to report when it ends.
    pub job: Option<JobId>,
    /// An Undo started from the Shelf, to report when it ends.
    pub undoing: bool,
    /// What the result line says when the drop succeeds.
    done_text: String,
}

/// Before the user chooses, the Shelf offers these folders.
fn default_pins() -> Vec<PathBuf> {
    [
        glib::UserDirectory::Documents,
        glib::UserDirectory::Downloads,
        glib::UserDirectory::Pictures,
        glib::UserDirectory::Desktop,
    ]
    .into_iter()
    .filter_map(glib::user_special_dir)
    .filter(|p| p.is_dir() && *p != glib::home_dir())
    .collect()
}

/// Visible items directly inside a folder.
fn count_items(folder: &Path) -> Option<usize> {
    let entries = std::fs::read_dir(folder).ok()?;
    Some(
        entries
            .flatten()
            .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .count(),
    )
}

fn kind_of(path: &Path) -> steward_core::Kind {
    if path.is_dir() {
        return steward_core::Kind::Folder;
    }
    let (content_type, _) = gio::content_type_guess(Some(name_of(path).as_str()), None);
    steward_core::Kind::from_content_type(&content_type)
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

impl Controller {
    fn shelf_pins(&self) -> Vec<PathBuf> {
        let store = app_data::store();
        let store = store.borrow();
        if store.shelf_pins_chosen() {
            store.shelf_pins().unwrap_or_default()
        } else {
            default_pins()
        }
    }

    /// `app.shelf`: opens the Shelf, or closes it when it is open.
    pub fn shelf_toggle(&mut self) {
        self.shelf.open = !self.shelf.open;
        self.mark(dirty::TOOLS);
    }

    pub fn shelf_close(&mut self) {
        if self.shelf.open {
            self.shelf.open = false;
            self.mark(dirty::TOOLS);
        }
    }

    /// Key 1 to 9: the main window opens that pinned folder.
    pub fn shelf_open_pin(&mut self, number: &str) {
        let Some(n) = number.parse::<usize>().ok().filter(|n| *n >= 1) else {
            return;
        };
        let Some(folder) = self.shelf_pins().get(n - 1).cloned() else {
            return;
        };
        self.shelf_open(&folder.display().to_string());
    }

    /// Opens a folder (or shows a file in its folder) in the main window.
    pub fn shelf_open(&mut self, path: &str) {
        self.shelf_close();
        self.go_to(path);
        self.emit(super::Event::Focus("raise"));
    }

    pub fn shelf_pin(&mut self, path: &str) {
        let path = PathBuf::from(path);
        if !path.is_dir() {
            return;
        }
        let mut pins = self.shelf_pins();
        if pins.contains(&path) {
            return;
        }
        if pins.len() >= shelf::MAX_PINS {
            self.toast(&gettext("The Shelf holds nine folders; unpin one first"));
            return;
        }
        pins.push(path);
        let _ = app_data::store().borrow_mut().set_shelf_pins(&pins);
        self.mark(dirty::TOOLS);
    }

    pub fn shelf_unpin(&mut self, number: &str) {
        let Some(n) = number.parse::<usize>().ok().filter(|n| *n >= 1) else {
            return;
        };
        let mut pins = self.shelf_pins();
        if n <= pins.len() {
            pins.remove(n - 1);
            let _ = app_data::store().borrow_mut().set_shelf_pins(&pins);
            self.mark(dirty::TOOLS);
        }
    }

    /// Puts files in Holding: newline-separated paths or file URIs. With no
    /// argument, the selection in the main window.
    pub fn shelf_hold(&mut self, list: &str) {
        let paths: Vec<PathBuf> = if list.trim().is_empty() {
            self.selected()
                .iter()
                .filter_map(|e| e.data().file.path())
                .collect()
        } else {
            list.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .filter_map(|l| gio::File::for_commandline_arg(l).path())
                .collect()
        };
        if paths.is_empty() {
            return;
        }
        let _ = app_data::store()
            .borrow_mut()
            .shelf_hold(&paths, app_data::now());
        self.mark(dirty::TOOLS);
    }

    pub fn shelf_unhold(&mut self, id: &str) {
        if let Ok(id) = id.parse() {
            let _ = app_data::store().borrow().shelf_unhold(id);
            self.mark(dirty::TOOLS);
        }
    }

    /// Clear: Holding forgets everything. The files stay where they are.
    pub fn shelf_clear(&mut self) {
        let _ = app_data::store().borrow().shelf_clear();
        self.mark(dirty::TOOLS);
    }

    /// A drop on pinned folder `n`: "n|move|uris" or "n|copy|uris"
    /// (newline-separated). Ctrl while dropping copies (DESIGN.md §4.4).
    pub fn shelf_drop(&mut self, value: &str) {
        let mut parts = value.splitn(3, '|');
        let (Some(n), Some(how), Some(list)) = (parts.next(), parts.next(), parts.next()) else {
            return;
        };
        let Some(n) = n.parse::<usize>().ok().filter(|n| *n >= 1) else {
            return;
        };
        let Some(dest) = self.shelf_pins().get(n - 1).cloned() else {
            return;
        };
        let sources: Vec<PathBuf> = list
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .filter_map(|l| gio::File::for_commandline_arg(l).path())
            .filter(|p| p.parent() != Some(dest.as_path()) && !dest.starts_with(p))
            .collect();
        if sources.is_empty() {
            return;
        }
        let what = match sources.as_slice() {
            [one] => format!("“{}”", name_of(one)),
            many => format::items(many.len() as u32),
        };
        let (request, verb, done) = if how == "copy" {
            (
                Request::Copy {
                    sources,
                    dest: dest.clone(),
                },
                gettext("Copy"),
                gettext("Copied"),
            )
        } else {
            (
                Request::Move {
                    sources,
                    dest: dest.clone(),
                },
                gettext("Move"),
                gettext("Moved"),
            )
        };
        let to = format!("{} “{}”", gettext("to"), name_of(&dest));
        let summary = format!("{verb} {what} {to}");
        self.shelf.done_text = format!("{done} {what} {to}");
        self.shelf.job = self.submit_job(request, summary);
        self.shelf.result = Some((gettext("Working…"), false));
        self.mark(dirty::TOOLS);
    }

    /// Undo from the result line.
    pub fn shelf_undo(&mut self) {
        self.shelf.undoing = true;
        self.undo();
    }

    /// An operation ended: the Shelf's result line says how, if it was ours.
    pub fn shelf_report(&mut self, job: JobId, undo: bool, text: Option<String>, can_undo: bool) {
        let ours = if undo {
            std::mem::take(&mut self.shelf.undoing)
        } else if self.shelf.job == Some(job) {
            self.shelf.job = None;
            true
        } else {
            false
        };
        if ours {
            // A drop that worked names what went where; anything else
            // (Undo, a failure) says what the operation reported.
            let text = if can_undo && !undo {
                Some(self.shelf.done_text.clone())
            } else {
                text
            };
            self.shelf.result = text.map(|t| (t, can_undo));
            self.mark(dirty::TOOLS);
        }
    }

    pub fn shelf_json(&self) -> Value {
        if !self.shelf.open {
            return json!({ "open": false });
        }
        let pins: Vec<Value> = self
            .shelf_pins()
            .iter()
            .enumerate()
            .map(|(i, p)| {
                json!({
                    "n": i + 1, "name": name_of(p), "path": p.display().to_string(),
                    "count": count_items(p), "exists": p.is_dir(),
                })
            })
            .collect();
        let held: Vec<Value> = app_data::store()
            .borrow()
            .shelf_held()
            .unwrap_or_default()
            .iter()
            .map(|h| {
                let kind = kind_of(&h.path);
                json!({
                    "id": h.id, "name": name_of(&h.path), "path": h.path.display().to_string(),
                    "uri": gio::File::for_path(&h.path).uri().to_string(),
                    "exists": h.path.exists(), "dir": h.path.is_dir(),
                    "kind": kind.id(), "badge": tint::badge(&name_of(&h.path), kind),
                })
            })
            .collect();
        let latest: Vec<Value> = shelf::latest_files(&downloads::downloads_folder(), 5)
            .iter()
            .map(|p| {
                let kind = kind_of(p);
                let mtime = std::fs::metadata(p)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                json!({
                    "name": name_of(p), "path": p.display().to_string(),
                    "uri": gio::File::for_path(p).uri().to_string(),
                    "kind": kind.id(), "badge": tint::badge(&name_of(p), kind), "date": format::date(mtime),
                })
            })
            .collect();
        let home = glib::home_dir();
        let mut quick = vec![
            json!({ "label": gettext("Home"), "icon": "home", "path": home.display().to_string() }),
        ];
        for (dir, label, icon) in [
            (glib::UserDirectory::Desktop, gettext("Desktop"), "desktop"),
            (
                glib::UserDirectory::Documents,
                gettext("Documents"),
                "documents",
            ),
        ] {
            if let Some(p) = glib::user_special_dir(dir).filter(|p| p.is_dir() && *p != home) {
                quick
                    .push(json!({ "label": label, "icon": icon, "path": p.display().to_string() }));
            }
        }
        quick.push(json!({ "label": gettext("Downloads"), "icon": "downloads",
                           "path": downloads::downloads_folder().display().to_string() }));
        json!({
            "open": true,
            "pins": pins,
            "held": held,
            "latest": latest,
            "quick": quick,
            "result": self.shelf.result.as_ref().map(|(text, undo)| json!({ "text": text, "undo": undo })),
            "home": paths::display_path(&home),
        })
    }

    #[cfg(feature = "e2e")]
    pub fn shelf_test_json(&self) -> Value {
        json!({
            "open": self.shelf.open,
            "pins": self.shelf_pins().iter().map(|p| json!({ "path": p.display().to_string(), "count": count_items(p) }))
                .collect::<Vec<_>>(),
            "held": app_data::store().borrow().shelf_held().unwrap_or_default().iter()
                .map(|h| h.path.display().to_string()).collect::<Vec<_>>(),
            "latest": shelf::latest_files(&downloads::downloads_folder(), 5).iter()
                .map(|p| name_of(p)).collect::<Vec<_>>(),
            "result": self.shelf.result.as_ref().map(|(t, u)| json!({ "text": t, "undo": u })),
        })
    }
}
