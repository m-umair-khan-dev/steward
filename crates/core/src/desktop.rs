//! Around the desktop (Phase 8): small pure helpers for the Shelf's global
//! shortcut (P8-T02), the autostart entry (P8-T03) and the Shell search
//! provider (P8-T04). The parts that talk to the session live in the app.

/// Where the Shelf's GNOME custom keybinding is kept (ADR 0014).
pub const KEYBINDING_PATH: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/cairn-shelf/";

/// A Qt key sequence ("Ctrl+Alt+Space") as a GTK accelerator for
/// gnome-settings-daemon ("<Control><Alt>space"). None if it has no key or
/// a modifier gsd does not know.
pub fn gtk_accel(qt: &str) -> Option<String> {
    let qt = qt.trim();
    if qt.is_empty() {
        return None;
    }
    // "Ctrl++" ends in the key "+".
    let (mods, key) = if let Some(rest) = qt.strip_suffix("++") {
        (rest, "+")
    } else {
        match qt.rsplit_once('+') {
            Some((mods, key)) => (mods, key),
            None => ("", qt),
        }
    };
    let mut out = String::new();
    for m in mods.split('+').filter(|m| !m.is_empty()) {
        out.push_str(match m.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => "<Control>",
            "alt" => "<Alt>",
            "shift" => "<Shift>",
            "meta" | "super" => "<Super>",
            _ => return None,
        });
    }
    let key = match key {
        "Space" => "space".to_string(),
        "Return" | "Enter" => "Return".to_string(),
        "Esc" | "Escape" => "Escape".to_string(),
        "Tab" => "Tab".to_string(),
        "Backspace" => "BackSpace".to_string(),
        "Del" | "Delete" => "Delete".to_string(),
        "Ins" | "Insert" => "Insert".to_string(),
        "Home" | "End" | "Left" | "Right" | "Up" | "Down" => key.to_string(),
        "PgUp" => "Page_Up".to_string(),
        "PgDown" => "Page_Down".to_string(),
        "," => "comma".to_string(),
        "." => "period".to_string(),
        "/" => "slash".to_string(),
        "-" => "minus".to_string(),
        "=" => "equal".to_string(),
        "+" => "plus".to_string(),
        ";" => "semicolon".to_string(),
        "'" => "apostrophe".to_string(),
        "`" => "grave".to_string(),
        "[" => "bracketleft".to_string(),
        "]" => "bracketright".to_string(),
        "\\" => "backslash".to_string(),
        k if k.len() > 1 && k.starts_with('F') && k[1..].parse::<u8>().is_ok() => k.to_string(),
        k if k.chars().count() == 1 && k.chars().all(|c| c.is_ascii_alphanumeric()) => {
            k.to_ascii_lowercase()
        }
        _ => return None,
    };
    out.push_str(&key);
    Some(out)
}

/// The XDG autostart entry that starts Cairn in the background at login
/// (P8-T03): no window, only the Shelf, the tray icon and search.
pub fn autostart_entry(exec: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Cairn\nComment=The Shelf and tray icon\n\
         Exec={exec} --background\nIcon=dev.cairn.Files\nTerminal=false\nNoDisplay=true\n\
         X-GNOME-Autostart-enabled=true\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qt_keys_become_gtk_accelerators() {
        assert_eq!(
            gtk_accel("Ctrl+Alt+Space").as_deref(),
            Some("<Control><Alt>space")
        );
        assert_eq!(
            gtk_accel("Meta+Shift+S").as_deref(),
            Some("<Super><Shift>s")
        );
        assert_eq!(gtk_accel("Ctrl++").as_deref(), Some("<Control>plus"));
        assert_eq!(gtk_accel("F12").as_deref(), Some("F12"));
        assert_eq!(
            gtk_accel("Ctrl+PgDown").as_deref(),
            Some("<Control>Page_Down")
        );
        assert_eq!(gtk_accel(""), None);
        assert_eq!(gtk_accel("Hyper+X"), None);
    }

    #[test]
    fn the_autostart_entry_starts_in_the_background() {
        let entry = autostart_entry("/usr/bin/cairn");
        assert!(entry.contains("Exec=/usr/bin/cairn --background\n"));
        assert!(entry.starts_with("[Desktop Entry]\n"));
    }
}
