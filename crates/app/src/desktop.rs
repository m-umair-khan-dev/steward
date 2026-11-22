//! Around the desktop (Phase 8): the Shelf's global shortcut (P8-T02).
//!
//! The shortcut is a GNOME custom keybinding that runs `cairn shelf`
//! (ADR 0014: the GlobalShortcuts portal route waits for spike P8-S1 on
//! real desktops). It is written only while the user has it turned on, and
//! removed when they turn it off. Without gnome-settings-daemon's schema
//! (another desktop) nothing is written.

use gio::prelude::*;

use cairn_core::desktop::{KEYBINDING_PATH, gtk_accel};

use crate::settings;

const MEDIA_KEYS: &str = "org.gnome.settings-daemon.plugins.media-keys";
const CUSTOM: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";

fn schema_installed(id: &str) -> bool {
    gio::SettingsSchemaSource::default().is_some_and(|source| source.lookup(id, true).is_some())
}

/// The command the keybinding runs: this program, asked for the Shelf.
fn command() -> String {
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "cairn".into());
    format!("{exe} shelf")
}

/// Makes GNOME's keybindings match the setting. Returns false when the
/// desktop has no custom keybindings (then nothing was written).
pub fn apply_shelf_shortcut() -> bool {
    if !schema_installed(MEDIA_KEYS) || !schema_installed(CUSTOM) {
        return false;
    }
    let ours = settings::get();
    let wanted = ours
        .boolean("shelf-shortcut-enabled")
        .then(|| gtk_accel(&ours.string("shelf-shortcut")))
        .flatten();
    let media = gio::Settings::new(MEDIA_KEYS);
    let mut list: Vec<String> = media
        .strv("custom-keybindings")
        .iter()
        .map(|p| p.to_string())
        .collect();
    let entry = gio::Settings::with_path(CUSTOM, KEYBINDING_PATH);
    match wanted {
        Some(binding) => {
            let _ = entry.set_string("name", "Cairn Shelf");
            let _ = entry.set_string("command", &command());
            let _ = entry.set_string("binding", &binding);
            if !list.iter().any(|p| p == KEYBINDING_PATH) {
                list.push(KEYBINDING_PATH.to_string());
                let refs: Vec<&str> = list.iter().map(String::as_str).collect();
                let _ = media.set_strv("custom-keybindings", refs);
            }
        }
        None => {
            if list.iter().any(|p| p == KEYBINDING_PATH) {
                list.retain(|p| p != KEYBINDING_PATH);
                let refs: Vec<&str> = list.iter().map(String::as_str).collect();
                let _ = media.set_strv("custom-keybindings", refs);
            }
            for key in ["name", "command", "binding"] {
                entry.reset(key);
            }
        }
    }
    gio::Settings::sync();
    true
}
