//! Cairn's engine: everything that is not widgets.
//!
//! This crate must never depend on GTK, so it can be tested headlessly and
//! shared by the app, the indexer and the command-line tool (DESIGN.md §7.1).
//! It uses GLib and GIO for files, mounts and models.

pub mod clean;
pub mod clip;
pub mod dupes;
pub mod entry;
pub mod folder;
pub mod git;
pub mod guard;
pub mod index;
pub mod kind;
pub mod natural;
pub mod openin;
pub mod ops;
pub mod packages;
pub mod places;
pub mod plan;
pub mod sort;
pub mod store;
pub mod suggest;
pub mod sysinfo;
pub mod tools;
pub mod workspace;

pub use entry::{EntryData, FileEntry};
pub use folder::{FolderModel, LoadState};
pub use kind::Kind;

/// The application ID, also used as the D-Bus name.
pub const APP_ID: &str = "dev.cairn.Files";

/// The D-Bus object path matching [`APP_ID`].
pub const APP_OBJECT_PATH: &str = "/dev/cairn/Files";
