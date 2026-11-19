//! Builds the QML module (dev.cairn.files) and the cxx-qt bridge, embeds the
//! icons, and compiles the settings schema for development builds.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

const QML_FILES: &[&str] = &[
    "qml/Main.qml",
    "qml/Theme.qml",
    "qml/Icon.qml",
    "qml/IconButton.qml",
    "qml/Segmented.qml",
    "qml/Sidebar.qml",
    "qml/Rail.qml",
    "qml/Header.qml",
    "qml/PathBar.qml",
    "qml/ChipRow.qml",
    "qml/FolderGrid.qml",
    "qml/FolderList.qml",
    "qml/GlassFolder.qml",
    "qml/FileCard.qml",
    "qml/PreviewPane.qml",
    "qml/StatusPage.qml",
    "qml/Toast.qml",
    "qml/Sheet.qml",
    "qml/AboutDialog.qml",
    "qml/PreferencesDialog.qml",
    "qml/ShortcutsDialog.qml",
    "qml/OpenWithDialog.qml",
    "qml/MenuPopup.qml",
];

fn main() {
    // Theme.qml holds the colour tokens (DESIGN.md §3.6) as a singleton.
    let files = QML_FILES.iter().map(|path| {
        let file = QmlFile::from(*path);
        if path.ends_with("Theme.qml") {
            file.singleton(true)
        } else {
            file
        }
    });
    CxxQtBuilder::new_qml_module(QmlModule::new("dev.cairn.files").qml_files(files))
        .qt_module("Quick")
        .qrc("qml/icons.qrc")
        .files(["src/bridge.rs"])
        .build();

    // Development and test builds load the settings schema from OUT_DIR, so
    // nothing has to be installed system-wide (see src/settings.rs).
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo"));
    let schema = manifest_dir.join("../../data/dev.cairn.Files.gschema.xml");
    println!("cargo:rerun-if-changed={}", schema.display());
    let schema_out = out_dir.join("schemas");
    fs::create_dir_all(&schema_out).expect("create OUT_DIR/schemas");
    fs::copy(&schema, schema_out.join("dev.cairn.Files.gschema.xml")).expect("copy schema");
    let status = Command::new("glib-compile-schemas")
        .arg("--strict")
        .arg(&schema_out)
        .status()
        .expect("run glib-compile-schemas");
    assert!(status.success(), "glib-compile-schemas failed");
}
