//! Cairn, a file manager for Ubuntu.

mod app_data;
mod application;
mod file_card;
mod folder_card;
mod folder_view;
mod format;
mod path_bar;
mod preferences;
mod preview_pane;
mod settings;
mod sidebar;
mod style;
#[cfg(feature = "e2e")]
mod test_iface;
mod thumbnails;
mod tint;
mod window;

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    // The guard runs before GTK or GLib start, so a refused start creates nothing
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

    application::run()
}
