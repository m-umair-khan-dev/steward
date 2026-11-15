//! Qt 6 / QML proof for Cairn. See README.md.

mod bridge;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};

fn main() {
    // Qt runs the GLib main loop on Linux. Own its default context so
    // cairn-core's GIO futures (glib::spawn_future_local) can be spawned.
    let context = glib::MainContext::default();
    let _guard = context.acquire().expect("own the GLib main context");

    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();
    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/dev/cairn/proof/qml/Main.qml"));
    }
    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
