//! Cairn's engine: everything that is not widgets.
//!
//! This crate must never depend on GTK, so it can be tested headlessly and
//! shared by the app, the indexer and the command-line tool (DESIGN.md §7.1).

pub mod guard;

/// The application ID, also used as the D-Bus name.
pub const APP_ID: &str = "dev.cairn.Files";

/// The D-Bus object path matching [`APP_ID`].
pub const APP_OBJECT_PATH: &str = "/dev/cairn/Files";
