//! Cairn, a file manager for Ubuntu.

mod actions;
mod app;
mod app_data;
mod bridge;
mod controller;
mod dbus;
mod desktop;
mod file_manager;
mod format;
mod icons;
mod launch;
mod paths;
mod report;
mod settings;
mod sidebar;
mod thumbnails;
mod tint;
mod tray;

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    // The guard runs before Qt or GLib start, so a refused start creates nothing
    // in the home folder (AGENTS.md rule 4, scenario E0-04).
    let guard_required =
        cairn_core::guard::is_required(cfg!(debug_assertions), cfg!(feature = "e2e"), |name| {
            std::env::var(name).ok()
        });
    if guard_required {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        if let Err(error) = cairn_core::guard::check(home.as_deref()) {
            eprintln!("cairn: {error}");
            return ExitCode::from(2);
        }
    }

    app::run()
}
