//! The standard places shown in the sidebar: home and the XDG user folders.

use std::path::PathBuf;

/// A sidebar place. `key` is stable and is what preferences refer to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub key: String,
    pub label: String,
    pub path: PathBuf,
    pub icon: &'static str,
}

/// Home first, then the user folders that exist, in a fixed order. Folders
/// that are not configured, or that point at home itself, are left out.
pub fn standard_places() -> Vec<Place> {
    let home = glib::home_dir();
    let mut places = vec![Place {
        key: "place:home".into(),
        label: "Home".into(),
        path: home.clone(),
        icon: "user-home-symbolic",
    }];
    let folders = [
        (
            glib::UserDirectory::Desktop,
            "desktop",
            "user-desktop-symbolic",
        ),
        (
            glib::UserDirectory::Documents,
            "documents",
            "folder-documents-symbolic",
        ),
        (
            glib::UserDirectory::Downloads,
            "downloads",
            "folder-download-symbolic",
        ),
        (glib::UserDirectory::Music, "music", "folder-music-symbolic"),
        (
            glib::UserDirectory::Pictures,
            "pictures",
            "folder-pictures-symbolic",
        ),
        (
            glib::UserDirectory::Videos,
            "videos",
            "folder-videos-symbolic",
        ),
    ];
    for (dir, key, icon) in folders {
        let Some(path) = glib::user_special_dir(dir) else {
            continue;
        };
        if path == home || !path.is_dir() {
            continue;
        }
        let label = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| key.into());
        places.push(Place {
            key: format!("place:{key}"),
            label,
            path,
            icon,
        });
    }
    places
}
