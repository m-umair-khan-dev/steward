//! Dual pane (P4-T07; reference: dual-pane.png). F3 splits the window: the
//! left pane is the usual view, the right pane a second folder with its own
//! selection. Tab switches the active pane, F5 copies and F6 moves the
//! active pane's selection into the other pane's folder. Both folders come
//! back after a restart.

use std::collections::HashSet;
use std::path::PathBuf;

use gettextrs::gettext;
use gio::prelude::*;
use serde_json::{Value, json};

use steward_core::ops::Request;
use steward_core::{FileEntry, FolderModel, LoadState};

use super::{Controller, dirty, later};
use crate::{format, paths, settings};

pub struct PaneState {
    pub open: bool,
    /// The right pane is the active one.
    pub right_active: bool,
    model: FolderModel,
    entries: Vec<FileEntry>,
    selection: HashSet<String>,
    anchor: Option<usize>,
    wired: bool,
}

impl Default for PaneState {
    fn default() -> Self {
        Self {
            open: false,
            right_active: false,
            model: FolderModel::new(),
            entries: Vec::new(),
            selection: HashSet::new(),
            anchor: None,
            wired: false,
        }
    }
}

impl std::fmt::Debug for PaneState {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("PaneState")
            .field("open", &self.open)
            .finish()
    }
}

fn name_of(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/".into())
}

impl Controller {
    fn wire_pane(&mut self) {
        if self.pane.wired {
            return;
        }
        self.pane.wired = true;
        self.pane
            .model
            .connect_state_changed(|_| later(|c| c.pane_recompute()));
        self.pane
            .model
            .store()
            .connect_items_changed(|_, _, _, _| later(|c| c.pane_recompute()));
    }

    fn pane_recompute(&mut self) {
        let store = self.pane.model.store();
        let mut entries: Vec<FileEntry> = (0..store.n_items())
            .filter_map(|i| store.item(i).and_downcast::<FileEntry>())
            .filter(|e| self.show_hidden || !e.data().is_hidden)
            .collect();
        entries.sort_by(|a, b| {
            b.is_dir()
                .cmp(&a.is_dir())
                .then_with(|| steward_core::natural::compare(a.name(), b.name()))
        });
        let uris: HashSet<String> = entries.iter().map(|e| e.data().uri()).collect();
        self.pane.selection.retain(|u| uris.contains(u));
        self.pane.entries = entries;
        self.mark(dirty::PANE);
    }

    /// The right pane's folder.
    pub fn pane_folder(&self) -> Option<PathBuf> {
        self.pane.model.folder().and_then(|f| f.path())
    }

    /// F3: split, or back to one pane.
    pub fn toggle_dual_pane(&mut self) {
        self.set_dual_pane(!self.pane.open, None);
    }

    pub fn set_dual_pane(&mut self, open: bool, right: Option<PathBuf>) {
        self.wire_pane();
        self.pane.open = open;
        self.pane.right_active = false;
        if open && (right.is_some() || self.pane.model.folder().is_none()) {
            let folder = right
                .or_else(|| self.current().and_then(|f| f.path()))
                .unwrap_or_else(glib::home_dir);
            self.pane_go(&folder.display().to_string());
        }
        let _ = settings::get().set_boolean("dual-pane", open);
        self.remember_panes();
        self.mark(dirty::PANE | dirty::FOLDER);
    }

    /// Both folders, so a restart brings the panes back (E4-10).
    pub fn remember_panes(&self) {
        let s = settings::get();
        if let Some(left) = self.current().and_then(|f| f.path()) {
            let _ = s.set_string("pane-left", &left.display().to_string());
        }
        if let Some(right) = self.pane_folder() {
            let _ = s.set_string("pane-right", &right.display().to_string());
        }
    }

    pub fn pane_go(&mut self, target: &str) {
        self.wire_pane();
        let path = paths::expand(target, None);
        if !path.is_dir() {
            return;
        }
        self.pane.selection.clear();
        self.pane.anchor = None;
        self.pane.entries.clear();
        self.pane.model.load(&gio::File::for_path(&path));
        self.remember_panes();
        self.mark(dirty::PANE);
    }

    /// Tab: the other pane becomes the active one.
    pub fn switch_pane(&mut self) {
        if self.pane.open {
            self.pane.right_active = !self.pane.right_active;
            self.mark(dirty::PANE | dirty::FOLDER);
        }
    }

    pub fn pane_activate(&mut self, right: bool) {
        if self.pane.open && self.pane.right_active != right {
            self.pane.right_active = right;
            self.mark(dirty::PANE | dirty::FOLDER);
        }
    }

    /// A click in the right pane (mode as for the left: 0 one, 1 add, 2 range).
    pub fn pane_pick(&mut self, pos: usize, mode: i32) {
        self.pane_activate(true);
        let Some(uri) = self.pane.entries.get(pos).map(|e| e.data().uri()) else {
            if mode == 0 {
                self.pane.selection.clear();
            }
            self.mark(dirty::PANE);
            return;
        };
        match mode {
            1 => {
                if !self.pane.selection.remove(&uri) {
                    self.pane.selection.insert(uri);
                }
            }
            2 => {
                let anchor = self.pane.anchor.unwrap_or(pos);
                let (a, b) = if anchor <= pos {
                    (anchor, pos)
                } else {
                    (pos, anchor)
                };
                self.pane.selection = self.pane.entries[a..=b]
                    .iter()
                    .map(|e| e.data().uri())
                    .collect();
            }
            _ => {
                self.pane.selection.clear();
                self.pane.selection.insert(uri);
            }
        }
        if mode != 2 {
            self.pane.anchor = Some(pos);
        }
        self.mark(dirty::PANE);
    }

    /// Enter or a double-click in the right pane: folders open there.
    pub fn pane_open(&mut self, pos: usize) {
        let Some(entry) = self.pane.entries.get(pos).cloned() else {
            return;
        };
        if entry.is_dir() {
            if let Some(path) = entry.data().file.path() {
                self.pane_go(&path.display().to_string());
            }
        } else {
            let uri = entry.data().uri();
            glib::spawn_future_local(async move {
                let _ =
                    gio::AppInfo::launch_default_for_uri_future(&uri, gio::AppLaunchContext::NONE)
                        .await;
            });
        }
    }

    /// Keys while the right pane is active.
    pub fn pane_key(&mut self, key: &str) {
        let count = self.pane.entries.len();
        match key {
            "up" | "down" | "home" | "end" if count > 0 => {
                let at = self.pane.anchor.unwrap_or(0) as i64;
                let next = match key {
                    "up" => at - 1,
                    "down" => at + 1,
                    "home" => 0,
                    _ => count as i64 - 1,
                };
                self.pane_pick(next.clamp(0, count as i64 - 1) as usize, 0);
            }
            "return" => {
                if let Some(at) = self.pane.anchor {
                    self.pane_open(at);
                }
            }
            "back" => {
                if let Some(parent) = self
                    .pane_folder()
                    .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                {
                    self.pane_go(&parent.display().to_string());
                }
            }
            _ => {}
        }
    }

    /// F5 (copy) and F6 (move): the active pane's selection into the other
    /// pane's folder, through the operations engine (so Undo works).
    pub fn across(&mut self, moving: bool) {
        if !self.pane.open {
            return;
        }
        let (sources, dest): (Vec<PathBuf>, Option<PathBuf>) = if self.pane.right_active {
            (
                self.pane
                    .entries
                    .iter()
                    .filter(|e| self.pane.selection.contains(&e.data().uri()))
                    .filter_map(|e| e.data().file.path())
                    .collect(),
                self.current().and_then(|f| f.path()),
            )
        } else {
            (
                self.selected()
                    .iter()
                    .filter_map(|e| e.data().file.path())
                    .collect(),
                self.pane_folder(),
            )
        };
        let Some(dest) = dest else { return };
        if sources.is_empty() {
            return;
        }
        let what = if sources.len() == 1 {
            format!("“{}”", name_of(&sources[0]))
        } else {
            format::items(sources.len() as u32)
        };
        let summary = format!(
            "{} {what} {} “{}”",
            if moving {
                gettext("Move")
            } else {
                gettext("Copy")
            },
            gettext("to"),
            name_of(&dest)
        );
        let request = if moving {
            Request::Move { sources, dest }
        } else {
            Request::Copy { sources, dest }
        };
        self.submit_request(request, summary);
    }

    /// Swaps the two panes' folders.
    pub fn swap_panes(&mut self) {
        let (Some(left), Some(right)) = (self.current().and_then(|f| f.path()), self.pane_folder())
        else {
            return;
        };
        self.navigate(&gio::File::for_path(&right), true);
        self.pane_go(&left.display().to_string());
    }

    pub fn pane_json(&self) -> Value {
        if !self.pane.open {
            return json!({ "open": false });
        }
        let folder = self.pane_folder();
        let selected: Vec<&FileEntry> = self
            .pane
            .entries
            .iter()
            .filter(|e| self.pane.selection.contains(&e.data().uri()))
            .collect();
        let bytes: u64 = selected
            .iter()
            .filter(|e| !e.is_dir())
            .map(|e| e.data().size)
            .sum();
        let left_selected = self.selected();
        let left_bytes: u64 = left_selected
            .iter()
            .filter(|e| !e.is_dir())
            .map(|e| e.data().size)
            .sum();
        let count = |n: usize, total: usize, b: u64| {
            if n == 0 {
                format::items(total as u32)
            } else {
                format!(
                    "{n} {} {total} {} · {}",
                    gettext("of"),
                    gettext("selected"),
                    format::size(b)
                )
            }
        };
        let crumbs = |path: Option<PathBuf>| -> Value {
            let Some(path) = path else { return json!([]) };
            let segments = paths::segments(&path, &gettext("Computer"));
            let skip = segments.len().saturating_sub(2);
            Value::Array(
                segments
                    .into_iter()
                    .skip(skip)
                    .map(|(label, p)| json!({ "label": label, "path": p.display().to_string() }))
                    .collect(),
            )
        };
        let target = if self.pane.right_active {
            self.current().and_then(|f| f.path())
        } else {
            folder.clone()
        };
        json!({
            "open": true,
            "rightActive": self.pane.right_active,
            "path": folder.as_ref().map(|p| p.display().to_string()),
            "crumbs": crumbs(folder.clone()),
            "leftCrumbs": crumbs(self.current().and_then(|f| f.path())),
            "summary": count(selected.len(), self.pane.entries.len(), bytes),
            "leftSummary": count(left_selected.len(), self.visible.len(), left_bytes),
            "target": target.as_deref().map(name_of).unwrap_or_default(),
            "loading": self.pane.model.state() == LoadState::Loading,
            "items": self.pane.entries.iter().enumerate().map(|(i, e)| {
                let data = e.data();
                json!({
                    "pos": i, "name": data.display_name, "uri": data.uri(), "dir": data.is_dir,
                    "kind": data.kind.id(),
                    "sizeText": if data.is_dir { "—".to_string() } else { format::size(data.size) },
                    "modified": format::date(data.modified),
                    "selected": self.pane.selection.contains(&data.uri()),
                })
            }).collect::<Vec<_>>(),
        })
    }

    #[cfg(feature = "e2e")]
    pub fn pane_test_json(&self) -> Value {
        json!({
            "open": self.pane.open,
            "right_active": self.pane.right_active,
            "path": self.pane_folder().map(|p| p.display().to_string()),
            "items": self.pane.entries.iter().map(|e| e.name().to_string()).collect::<Vec<_>>(),
            "selection": self.pane.entries.iter()
                .filter(|e| self.pane.selection.contains(&e.data().uri()))
                .map(|e| e.name().to_string()).collect::<Vec<_>>(),
        })
    }
}
