//! Starts the Qt application and loads the QML window (ADR 0009).

use std::process::ExitCode;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

/// The gettext domain. Translations are wired up in Phase 6.
const GETTEXT_DOMAIN: &str = "cairn";

pub fn run() -> ExitCode {
    init_gettext();
    // Qt runs GLib's main loop on Linux. Owning its default context lets the
    // GIO-based engine spawn its futures on it.
    let context = glib::MainContext::default();
    let _guard = context.acquire().expect("own the GLib main context");

    let mut app = QGuiApplication::new();
    if let Some(mut app) = app.as_mut() {
        app.as_mut().set_application_name(&QString::from("cairn"));
        app.as_mut()
            .set_application_display_name(&QString::from("Cairn"));
        app.as_mut()
            .set_application_version(&QString::from(env!("CARGO_PKG_VERSION")));
        app.set_organization_domain(&QString::from("cairn.dev"));
    }
    let mut engine = QQmlApplicationEngine::new();
    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/dev/cairn/files/qml/Main.qml"));
    }
    let code = app.as_mut().map(|app| app.exec()).unwrap_or(1);
    // Make sure every setting reached dconf before the process ends.
    gio::Settings::sync();
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}

fn init_gettext() {
    let _ = gettextrs::bindtextdomain(GETTEXT_DOMAIN, "/usr/share/locale");
    let _ = gettextrs::bind_textdomain_codeset(GETTEXT_DOMAIN, "UTF-8");
    let _ = gettextrs::textdomain(GETTEXT_DOMAIN);
}
