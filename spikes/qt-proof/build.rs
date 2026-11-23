use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("dev.steward.proof").qml_files([
        "qml/Main.qml",
        "qml/GlassFolder.qml",
        "qml/FileCard.qml",
        "qml/Segmented.qml",
    ]))
    .qt_module("Quick")
    .files(["src/bridge.rs"])
    .build();
}
