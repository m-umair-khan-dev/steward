//! Capability detection (P9-T01; DESIGN.md D6): when Cairn runs inside a
//! sandbox (Flatpak, Snap), it cannot ask the host's package tools or read
//! the host's package databases. Features that need them say so, instead
//! of showing nothing; everything that works on the user's own files goes
//! on as usual.

use std::path::Path;

/// The sandbox Cairn runs in, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sandbox {
    Flatpak,
    Snap,
    /// Tests: simulated by the test switch (E9-01).
    Simulated,
}

impl Sandbox {
    pub fn name(self) -> &'static str {
        match self {
            Sandbox::Flatpak => "Flatpak",
            Sandbox::Snap => "Snap",
            Sandbox::Simulated => "a sandbox",
        }
    }
}

/// What runs on the host and so is out of reach from a sandbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Needs {
    /// The Installed page: dpkg, snap and flatpak lists.
    PackageLists,
    /// Smart Clean's "already installed" installers: the dpkg database.
    PackageDatabase,
    /// Smart Clean's leftovers of removed apps: snap and flatpak lists.
    AppLists,
    /// The "Needs administrator" commands: apt, journalctl, snap, docker.
    HostCommands,
}

/// Detects the sandbox from the marker files and variables Flatpak and
/// Snap set. `simulate` is the test switch (only honoured by test builds).
pub fn detect(root: &Path, snap_var: Option<&str>, simulate: bool) -> Option<Sandbox> {
    if simulate {
        Some(Sandbox::Simulated)
    } else if root.join(".flatpak-info").exists() {
        Some(Sandbox::Flatpak)
    } else if snap_var.is_some_and(|v| !v.is_empty()) {
        Some(Sandbox::Snap)
    } else {
        None
    }
}

/// Whether something that needs `needs` works in `sandbox`.
pub fn available(sandbox: Option<Sandbox>, needs: Needs) -> bool {
    let _ = needs; // every host need is out of reach from any sandbox today
    sandbox.is_none()
}

/// The Smart Clean cards that need the host (by card id).
pub fn card_needs(card: &str) -> Option<Needs> {
    match card {
        "installers" => Some(Needs::PackageDatabase),
        "leftovers" => Some(Needs::AppLists),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection() {
        let dir = std::env::temp_dir().join(format!("cairn-sandbox-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(detect(&dir, None, false), None);
        assert_eq!(
            detect(&dir, Some("/snap/cairn/1"), false),
            Some(Sandbox::Snap)
        );
        assert_eq!(detect(&dir, None, true), Some(Sandbox::Simulated));
        std::fs::write(dir.join(".flatpak-info"), "[Application]\n").unwrap();
        assert_eq!(detect(&dir, Some(""), false), Some(Sandbox::Flatpak));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn host_features_are_unavailable_in_a_sandbox() {
        assert!(available(None, Needs::PackageLists));
        assert!(!available(Some(Sandbox::Flatpak), Needs::PackageLists));
        assert_eq!(card_needs("installers"), Some(Needs::PackageDatabase));
        assert_eq!(card_needs("duplicates"), None);
    }
}
