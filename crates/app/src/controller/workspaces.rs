//! Workspaces in the window (P4-T08, P4-T09; reference: workspaces.png):
//! the sidebar section, the workspace page, adding items, Locate…, and
//! Alt+1 to Alt+9.

use std::path::{Path, PathBuf};

use gettextrs::gettext;
use gio::prelude::*;
use serde_json::{Value, json};

use cairn_core::Kind;
use cairn_core::workspace::{Item, Where};

use super::{Controller, Event, dirty};
use crate::{app_data, format, paths};

/// Colours new workspaces take in turn (kind tints).
const COLORS: [&str; 8] = [
    "archives",
    "pdfs",
    "folders",
    "documents",
    "images",
    "videos",
    "code",
    "audio",
];

#[derive(Default, Debug)]
pub struct WorkspaceState {
    /// The workspace page on screen.
    pub open: Option<i64>,
    /// "overview", "folders", "images" or "documents".
    tab: String,
}

struct Shown {
    item: Item,
    place: Where,
    path: PathBuf,
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn kind_of(path: &Path, is_dir: bool) -> Kind {
    if is_dir {
        Kind::Folder
    } else {
        cairn_core::plan::kind_of(&name_of(path))
    }
}

impl Controller {
    fn workspace_items_now(&self, id: i64) -> Vec<Shown> {
        let store = app_data::store();
        let store = store.borrow();
        let index = if self.index.running {
            self.index.reader_ref()
        } else {
            None
        };
        store
            .workspace_items(id)
            .unwrap_or_default()
            .into_iter()
            .map(|item| {
                let place = store.resolve_item(&item, index);
                let path = match &place {
                    Where::Here(p) | Where::Moved(p) => p.clone(),
                    _ => item.path.clone(),
                };
                Shown { item, place, path }
            })
            .collect()
    }

    pub fn create_workspace(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        let store = app_data::store();
        let count = store.borrow().workspaces().map(|w| w.len()).unwrap_or(0);
        let id = store
            .borrow()
            .create_workspace(name, COLORS[count % COLORS.len()]);
        drop(store);
        if let Ok(id) = id {
            self.close_dialog_if("workspace-new");
            self.open_workspace(id);
        }
        self.mark(dirty::SIDEBAR);
    }

    fn close_dialog_if(&mut self, kind: &str) {
        if self.dialog == kind {
            self.close_dialog();
        }
    }

    /// Opens by id, or by place in the list ("#1" … "#9", Alt+1 … Alt+9).
    pub fn open_workspace_arg(&mut self, arg: &str) {
        let id = if let Some(n) = arg.strip_prefix('#') {
            let n: usize = n.parse().unwrap_or(0);
            app_data::store()
                .borrow()
                .workspaces()
                .ok()
                .and_then(|w| w.get(n.saturating_sub(1)).map(|w| w.id))
        } else {
            arg.parse().ok()
        };
        if let Some(id) = id {
            self.open_workspace(id);
        }
    }

    pub fn open_workspace(&mut self, id: i64) {
        if app_data::store()
            .borrow()
            .workspace(id)
            .ok()
            .flatten()
            .is_none()
        {
            return;
        }
        self.workspace.open = Some(id);
        self.workspace.tab = "overview".into();
        self.close_search();
        self.mark(dirty::WORKSPACE | dirty::SIDEBAR);
    }

    pub fn close_workspace(&mut self) {
        if self.workspace.open.take().is_some() {
            self.mark(dirty::WORKSPACE | dirty::SIDEBAR);
        }
    }

    pub fn workspace_tab(&mut self, tab: &str) {
        self.workspace.tab = tab.to_string();
        self.mark(dirty::WORKSPACE);
    }

    /// Adds the selection (or `paths`) to a workspace. Nothing moves.
    pub fn add_to_workspace(&mut self, id: i64, paths: Vec<PathBuf>) {
        let paths = if paths.is_empty() {
            self.selected()
                .iter()
                .filter_map(|e| e.data().file.path())
                .collect()
        } else {
            paths
        };
        let store = app_data::store();
        let added = paths
            .iter()
            .filter(|p| matches!(store.borrow().add_to_workspace(id, p), Ok(Some(_))))
            .count();
        let name = store
            .borrow()
            .workspace(id)
            .ok()
            .flatten()
            .map(|w| w.name)
            .unwrap_or_default();
        drop(store);
        if added > 0 {
            self.emit(Event::Toast(
                format!(
                    "{} {} {} “{name}”",
                    gettext("Added"),
                    format::items(added as u32),
                    gettext("to")
                ),
                false,
            ));
        }
        self.mark(dirty::WORKSPACE | dirty::SIDEBAR);
    }

    /// `arg` is "workspace-id" or "workspace-id:path".
    pub fn add_to_workspace_arg(&mut self, arg: &str) {
        let (id, path) = arg
            .split_once(':')
            .map(|(i, p)| (i, Some(p)))
            .unwrap_or((arg, None));
        let Ok(id) = id.parse() else { return };
        let paths = path
            .map(|p| vec![paths::expand(p, None)])
            .unwrap_or_default();
        self.add_to_workspace(id, paths);
    }

    pub fn remove_from_workspace(&mut self, item: &str) {
        if let Ok(item) = item.parse() {
            let _ = app_data::store().borrow().remove_from_workspace(item);
            self.mark(dirty::WORKSPACE);
        }
    }

    /// Locate…: "item-id:path".
    pub fn locate_item(&mut self, arg: &str) {
        let Some((item, path)) = arg.split_once(':') else {
            return;
        };
        let Ok(item) = item.parse() else { return };
        let path = paths::expand(path, None);
        let path = path
            .strip_prefix("file://")
            .map(Path::to_path_buf)
            .unwrap_or(path);
        if !matches!(
            app_data::store().borrow().relocate_item(item, &path),
            Ok(true)
        ) {
            self.emit(Event::Toast(gettext("That place does not exist"), false));
        }
        self.mark(dirty::WORKSPACE);
    }

    /// Edit: "id|name|description|colour".
    pub fn edit_workspace(&mut self, arg: &str) {
        let parts: Vec<&str> = arg.splitn(4, '|').collect();
        let [id, name, description, color] = parts.as_slice() else {
            return;
        };
        let Ok(id) = id.parse() else { return };
        if name.trim().is_empty() {
            return;
        }
        let _ =
            app_data::store()
                .borrow()
                .update_workspace(id, name.trim(), description.trim(), color);
        self.close_dialog_if("workspace-edit");
        self.mark(dirty::WORKSPACE | dirty::SIDEBAR);
    }

    pub fn ask_edit_workspace(&mut self, id: &str) {
        let Ok(id) = id.parse::<i64>() else { return };
        let Some(ws) = app_data::store().borrow().workspace(id).ok().flatten() else {
            return;
        };
        self.dialog_data = json!({ "id": ws.id, "name": ws.name, "description": ws.description, "color": ws.color,
                                   "colors": COLORS });
        self.show_dialog("workspace-edit");
    }

    pub fn ask_new_workspace(&mut self) {
        self.dialog_data = json!({ "colors": COLORS });
        self.show_dialog("workspace-new");
    }

    pub fn delete_workspace(&mut self, id: &str) {
        let Ok(id) = id.parse::<i64>() else { return };
        let _ = app_data::store().borrow().delete_workspace(id);
        if self.workspace.open == Some(id) {
            self.workspace.open = None;
        }
        self.close_dialog_if("workspace-edit");
        self.mark(dirty::WORKSPACE | dirty::SIDEBAR);
    }

    pub fn workspaces_json(&self) -> Value {
        let list = app_data::store().borrow().workspaces().unwrap_or_default();
        Value::Array(
            list.iter()
                .enumerate()
                .map(|(i, w)| {
                    json!({ "id": w.id, "name": w.name, "color": w.color, "number": i + 1,
                                      "open": self.workspace.open == Some(w.id) })
                })
                .collect(),
        )
    }

    pub fn workspace_json(&self) -> Value {
        let Some(id) = self.workspace.open else {
            return json!({ "open": false });
        };
        let Some(ws) = app_data::store().borrow().workspace(id).ok().flatten() else {
            return json!({ "open": false });
        };
        let shown = self.workspace_items_now(id);
        let items: Vec<Value> = shown
            .iter()
            .map(|s| {
                let kind = kind_of(&s.path, s.item.is_dir);
                let (state, label) = match &s.place {
                    Where::Here(_) | Where::Moved(_) => ("here", None),
                    Where::Unplugged(drive) => {
                        ("unplugged", Some(format!("{} {drive}", gettext("On"))))
                    }
                    Where::Missing => ("missing", Some(gettext("Not found"))),
                };
                let numbers = if s.item.is_dir {
                    self.folder_numbers(&s.path)
                } else {
                    None
                };
                let meta = match (&s.place, numbers) {
                    (Where::Here(_) | Where::Moved(_), Some((items, bytes))) => {
                        format!("{} · {}", format::items(items as u32), format::size(bytes))
                    }
                    (Where::Here(_) | Where::Moved(_), None) if !s.item.is_dir => {
                        format::size(s.path.metadata().map(|m| m.len()).unwrap_or(0))
                    }
                    _ => String::new(),
                };
                let place = s.item.volume.clone().unwrap_or_else(|| {
                    s.path.parent().map(paths::display_path).unwrap_or_default()
                });
                let modified = s
                    .path
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| format::date(d.as_secs() as i64))
                    .unwrap_or_default();
                json!({
                    "id": s.item.id, "name": name_of(&s.path), "path": s.path.display().to_string(),
                    "uri": gio::File::for_path(&s.path).uri().to_string(),
                    "dir": s.item.is_dir, "kind": kind.id(), "state": state, "label": label,
                    "where": place, "meta": meta, "date": modified,
                })
            })
            .collect();
        let count = |kind: &str| {
            items
                .iter()
                .filter(|i| {
                    let k = i["kind"].as_str().unwrap_or("");
                    match kind {
                        "folders" => i["dir"] == true,
                        "images" => k == "images" || k == "videos",
                        "documents" => !(i["dir"] == true) && k != "images" && k != "videos",
                        _ => true,
                    }
                })
                .count()
        };
        let tab = if self.workspace.tab.is_empty() {
            "overview"
        } else {
            &self.workspace.tab
        };
        let folders = count("folders");
        let files = items.len() - folders;
        let bytes: u64 = shown
            .iter()
            .filter(|s| matches!(s.place, Where::Here(_) | Where::Moved(_)))
            .map(|s| {
                if s.item.is_dir {
                    self.folder_numbers(&s.path).map(|n| n.1).unwrap_or(0)
                } else {
                    s.path.metadata().map(|m| m.len()).unwrap_or(0)
                }
            })
            .sum();
        json!({
            "open": true, "id": ws.id, "name": ws.name, "description": ws.description, "color": ws.color,
            "summary": format!("{}, {} · {}", format::folders(folders as u32), format::files(files as u32), format::size(bytes)),
            "tab": tab,
            "tabs": [
                { "id": "overview", "label": gettext("Overview"), "count": items.len() },
                { "id": "folders", "label": gettext("Folders"), "count": folders },
                { "id": "images", "label": gettext("Images"), "count": count("images") },
                { "id": "documents", "label": gettext("Documents"), "count": count("documents") },
            ],
            "items": items.into_iter().filter(|i| {
                let k = i["kind"].as_str().unwrap_or("");
                match tab {
                    "folders" => i["dir"] == true,
                    "images" => k == "images" || k == "videos",
                    "documents" => !(i["dir"] == true) && k != "images" && k != "videos",
                    _ => true,
                }
            }).collect::<Vec<_>>(),
        })
    }

    #[cfg(feature = "e2e")]
    pub fn workspace_test_json(&self) -> Value {
        let page = self.workspace_json();
        json!({
            "open": page["open"], "name": page["name"], "id": page["id"],
            "workspaces": self.workspaces_json().as_array().map(|l| l.iter().map(|w| w["name"].clone()).collect::<Vec<_>>()),
            "items": page["items"].as_array().map(|l| l.iter().map(|i| json!({
                "id": i["id"], "name": i["name"], "where": i["where"], "state": i["state"], "path": i["path"],
            })).collect::<Vec<_>>()),
        })
    }
}
