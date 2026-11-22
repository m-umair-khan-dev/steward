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
    "qml/RenameField.qml",
    "qml/PillButton.qml",
    "qml/ClashDialog.qml",
    "qml/EmptyTrashDialog.qml",
    "qml/TrashBar.qml",
    "qml/JobsPopup.qml",
    "qml/DragSource.qml",
    "qml/FolderDrop.qml",
    "qml/SearchOverlay.qml",
    "qml/PeekOverlay.qml",
    "qml/TidyDialog.qml",
    "qml/RightPane.qml",
    "qml/PaneHeader.qml",
    "qml/MiniFolder.qml",
    "qml/WorkspaceDialog.qml",
    "qml/WorkspacePage.qml",
    "qml/FolderStyleDialog.qml",
    "qml/MenuPopup.qml",
    "qml/FxShadow.qml",
    "qml/FxBlur.qml",
    "qml/SymbolFolder.qml",
    "qml/TickBox.qml",
    "qml/CleanPage.qml",
    "qml/InstalledPage.qml",
    "qml/AboutPage.qml",
    "qml/AgentsPage.qml",
    "qml/MarkdownPage.qml",
    "qml/BookmarksPage.qml",
    "qml/ShelfWindow.qml",
    "qml/AddLinkDialog.qml",
    "qml/BookmarkDialog.qml",
    "qml/CollectionDialog.qml",
    "qml/StopServerDialog.qml",
    "qml/WelcomeDialog.qml",
    "qml/WhatsNewDialog.qml",
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
    let builder = CxxQtBuilder::new_qml_module(QmlModule::new("dev.cairn.files").qml_files(files))
        .qt_module("Quick")
        .qrc("qml/icons.qrc")
        // Qt 6.5+ effects, outside the QML module (see qml/fx.qrc).
        .qrc("qml/fx.qrc")
        .qrc("qml/local.qrc")
        .files(["src/bridge.rs"]);
    // SAFETY: `cc_builder` is unsafe because changing compiler flags could
    // make the generated code unsound. We only add an include folder and our
    // own source file (the clipboard helper); no flags change.
    #[allow(unsafe_code)]
    let builder = unsafe {
        builder.cc_builder(|cc| {
            cc.include("cpp");
            cc.file("cpp/clipboard.cpp");
        })
    };
    builder.build();
    println!("cargo:rerun-if-changed=cpp");

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

    // Icons as text, for recolouring them on Qt 6.4 (src/icons.rs).
    let icons_dir = manifest_dir.join("qml/icons");
    println!("cargo:rerun-if-changed={}", icons_dir.display());
    let mut names: Vec<String> = fs::read_dir(&icons_dir)
        .expect("qml/icons")
        .flatten()
        .filter_map(|e| {
            e.file_name()
                .to_str()?
                .strip_suffix(".svg")
                .map(str::to_string)
        })
        .collect();
    names.sort();
    let table: String = names
        .iter()
        .map(|n| {
            format!(
                "    ({n:?}, include_str!({:?})),\n",
                icons_dir.join(format!("{n}.svg")).display().to_string()
            )
        })
        .collect();
    fs::write(
        out_dir.join("icons.rs"),
        format!("pub const ICONS: &[(&str, &str)] = &[\n{table}];\n"),
    )
    .expect("write icons.rs");

    // Translations (P6-T04): po/<lang>.po -> OUT_DIR/locale/<lang>/LC_MESSAGES/cairn.mo
    // for development and test builds (installed builds use /usr/share/locale).
    let po = manifest_dir.join("../../po");
    println!("cargo:rerun-if-changed={}", po.display());
    for entry in fs::read_dir(&po).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "po") {
            continue;
        }
        println!("cargo:rerun-if-changed={}", path.display());
        let lang = path
            .file_stem()
            .expect("a po file has a name")
            .to_string_lossy()
            .into_owned();
        let dir = out_dir.join("locale").join(&lang).join("LC_MESSAGES");
        fs::create_dir_all(&dir).expect("create the locale folder");
        let status = Command::new("msgfmt")
            .arg("--check")
            .arg("-o")
            .arg(dir.join("cairn.mo"))
            .arg(&path)
            .status()
            .expect("run msgfmt");
        assert!(status.success(), "msgfmt failed for {}", path.display());
    }
}
