//! Sidebar data (P1-T06): Favorites (home and the XDG folders) and Locations
//! (mounted drives and shares), with the order and hidden entries the user
//! chose, kept in app.db. The QML sidebar draws what this returns.

use gettextrs::gettext;
use gio::prelude::*;

use steward_core::places;

use crate::app_data;

#[derive(Debug, Clone)]
pub struct SidebarEntry {
    pub key: String,
    pub label: String,
    /// "favorites" or "locations".
    pub section: &'static str,
    pub file: gio::File,
    /// Name of one of our icons (qml/icons/<name>.svg).
    pub icon: &'static str,
    pub can_eject: bool,
}

fn place_icon(key: &str) -> &'static str {
    match key {
        "place:home" => "home",
        "place:desktop" => "desktop",
        "place:documents" => "documents",
        "place:downloads" => "downloads",
        "place:music" => "music",
        "place:pictures" => "pictures",
        "place:videos" => "videos",
        "place:trash" => "trash",
        _ => "folder",
    }
}

fn default_entries() -> Vec<SidebarEntry> {
    let mut entries: Vec<SidebarEntry> = places::standard_places()
        .into_iter()
        .map(|place| SidebarEntry {
            icon: place_icon(&place.key),
            label: if place.key == "place:home" {
                gettext("Home")
            } else {
                place.label
            },
            // The Downloads entry follows the preference (P4-T06).
            file: if place.key == "place:downloads" {
                gio::File::for_path(crate::controller::downloads_folder())
            } else {
                gio::File::for_path(&place.path)
            },
            key: place.key,
            section: "favorites",
            can_eject: false,
        })
        .collect();
    entries.push(SidebarEntry {
        key: "place:trash".into(),
        label: gettext("Trash"),
        section: "favorites",
        file: gio::File::for_path(steward_core::ops::Trash::default_home().join("files")),
        icon: "trash",
        can_eject: false,
    });
    for mount in gio::VolumeMonitor::get().mounts() {
        if mount.is_shadowed() || !(mount.can_unmount() || mount.can_eject()) {
            continue;
        }
        let root = mount.root();
        entries.push(SidebarEntry {
            key: format!("device:{}", root.uri()),
            label: mount.name().to_string(),
            section: "locations",
            file: root,
            icon: "drive",
            can_eject: true,
        });
    }
    entries
}

/// The entries to show, in the user's order, and the keys they hid.
pub fn load() -> (Vec<SidebarEntry>, Vec<String>) {
    let choices = app_data::store()
        .borrow()
        .sidebar_choices()
        .unwrap_or_default();
    let hidden: Vec<String> = choices
        .iter()
        .filter(|c| c.hidden)
        .map(|c| c.key.clone())
        .collect();
    let position = |key: &str| {
        choices
            .iter()
            .find(|c| c.key == key)
            .and_then(|c| c.position)
    };
    let mut indexed: Vec<(usize, SidebarEntry)> =
        default_entries().into_iter().enumerate().collect();
    indexed.sort_by_key(|(index, entry)| match position(&entry.key) {
        Some(p) => (0, p, *index as i64),
        None => (1, 0, *index as i64),
    });
    let entries = indexed
        .into_iter()
        .map(|(_, e)| e)
        .filter(|e| !hidden.contains(&e.key))
        .collect();
    (entries, hidden)
}

pub fn hide(key: &str) {
    let _ = app_data::store().borrow().set_sidebar_hidden(key, true);
}

/// Brings back an entry the user hid (Preferences, or Undo on the toast).
pub fn show(key: &str) {
    let _ = app_data::store().borrow().set_sidebar_hidden(key, false);
}

/// Every Favorites entry in the user's order, hidden ones included, as
/// (key, label, icon, hidden): Preferences lists them with a switch each.
pub fn favorites() -> Vec<(String, String, &'static str, bool)> {
    let (shown, hidden) = load();
    let mut out: Vec<_> = shown
        .into_iter()
        .filter(|e| e.section == "favorites")
        .map(|e| (e.key, e.label, e.icon, false))
        .collect();
    // Hidden entries keep their default place among the others.
    for (index, entry) in default_entries().into_iter().enumerate() {
        if entry.section == "favorites" && hidden.contains(&entry.key) {
            let at = index.min(out.len());
            out.insert(at, (entry.key, entry.label, entry.icon, true));
        }
    }
    out
}

pub fn move_by(entries: &[SidebarEntry], key: &str, delta: i32) {
    let mut keys: Vec<String> = entries.iter().map(|e| e.key.clone()).collect();
    let Some(index) = keys.iter().position(|k| k == key) else {
        return;
    };
    let target = (index as i32 + delta).clamp(0, keys.len() as i32 - 1) as usize;
    let item = keys.remove(index);
    keys.insert(target, item);
    let _ = app_data::store().borrow_mut().set_sidebar_order(&keys);
}

pub fn reset() {
    let _ = app_data::store().borrow().reset_sidebar();
}

/// Ejects or unmounts the drive behind a "device:" entry.
pub fn eject(file: &gio::File) {
    let Ok(mount) = file.find_enclosing_mount(gio::Cancellable::NONE) else {
        return;
    };
    glib::spawn_future_local(async move {
        let result = if mount.can_eject() {
            mount
                .eject_with_operation_future(
                    gio::MountUnmountFlags::NONE,
                    gio::MountOperation::NONE,
                )
                .await
        } else {
            mount
                .unmount_with_operation_future(
                    gio::MountUnmountFlags::NONE,
                    gio::MountOperation::NONE,
                )
                .await
        };
        if let Err(error) = result {
            glib::g_warning!("steward", "could not eject: {error}");
        }
    });
}
