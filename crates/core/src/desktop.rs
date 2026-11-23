//! Around the desktop (Phase 8): small pure helpers for the Shelf's global
//! shortcut (P8-T02), the autostart entry (P8-T03) and the Shell search
//! provider (P8-T04). The parts that talk to the session live in the app.

/// Where the Shelf's GNOME custom keybinding is kept (ADR 0014).
pub const KEYBINDING_PATH: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/steward-shelf/";

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

/// The XDG autostart entry that starts Steward in the background at login
/// (P8-T03): no window, only the Shelf, the tray icon and search.
pub fn autostart_entry(exec: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Steward\nComment=The Shelf and tray icon\n\
         Exec={exec} --background\nIcon=io.github.m_umair_khan_dev.steward\nTerminal=false\nNoDisplay=true\n\
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
        let entry = autostart_entry("/usr/bin/steward");
        assert!(entry.contains("Exec=/usr/bin/steward --background\n"));
        assert!(entry.starts_with("[Desktop Entry]\n"));
    }
}

/// The desktop's accent colour (Settings › Appearance), as a hex colour.
/// GNOME 47 and later keep it in `accent-color`; Ubuntu 24.04 keeps it in
/// the Yaru theme's name ("Yaru-blue-dark"). None: the desktop has no
/// accent (then the app uses the platform palette).
pub fn system_accent(accent_color: Option<&str>, gtk_theme: Option<&str>) -> Option<&'static str> {
    // libadwaita's accent colours (GNOME 47+), which Ubuntu's Settings sets.
    let gnome = match accent_color.unwrap_or("") {
        "blue" => Some("#3584e4"),
        "teal" => Some("#2190a4"),
        "green" => Some("#3a944a"),
        "yellow" => Some("#c88800"),
        "orange" => Some("#ed5b00"),
        "red" => Some("#e62d42"),
        "pink" => Some("#d56199"),
        "purple" => Some("#9141ac"),
        "slate" => Some("#6f8396"),
        _ => None,
    };
    if gnome.is_some() {
        return gnome;
    }
    // Ubuntu 24.04: Yaru's accent variants.
    let theme = gtk_theme?.strip_prefix("Yaru")?;
    let variant = theme
        .trim_start_matches('-')
        .trim_end_matches("-dark")
        .trim_end_matches("dark");
    Some(match variant {
        "" => "#e95420", // Ubuntu orange
        "bark" => "#787859",
        "sage" => "#657b69",
        "olive" => "#4b8501",
        "viridian" => "#03875b",
        "prussiangreen" => "#308280",
        "blue" => "#0073e5",
        "purple" => "#7764d8",
        "magenta" => "#b34cb3",
        "red" => "#da3450",
        _ => return None,
    })
}

#[cfg(test)]
mod accent_tests {
    use super::system_accent;

    #[test]
    fn the_desktop_accent() {
        assert_eq!(
            system_accent(Some("teal"), Some("Yaru")),
            Some("#2190a4"),
            "GNOME's setting wins"
        );
        assert_eq!(
            system_accent(Some("default"), Some("Yaru-blue-dark")),
            Some("#0073e5")
        );
        assert_eq!(system_accent(None, Some("Yaru")), Some("#e95420"));
        assert_eq!(system_accent(None, Some("Yaru-dark")), Some("#e95420"));
        assert_eq!(system_accent(None, Some("Adwaita")), None);
    }
}
