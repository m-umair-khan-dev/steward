//! Starts the Qt application and loads the QML window (ADR 0009).

use std::process::ExitCode;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

/// The gettext domain (po/cairn.pot, P6-T04).
const GETTEXT_DOMAIN: &str = "cairn";

pub fn run() -> ExitCode {
    init_gettext();
    // A second launch hands its request to the running Cairn (P6-T06).
    let args: Vec<String> = std::env::args().skip(1).collect();
    let requests = crate::launch::parse(&args);
    if crate::launch::forward(&requests) {
        return ExitCode::SUCCESS;
    }
    crate::launch::set_startup(requests);
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
        app.as_mut()
            .set_organization_domain(&QString::from("cairn.dev"));
        // The Wayland app ID, so the shell matches the window to its desktop file.
        QGuiApplication::set_desktop_file_name(&QString::from(cairn_core::APP_ID));
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

/// Where the message catalogs are: installed builds use the system's; test
/// and development builds use the catalogs build.rs compiled from po/.
fn locale_dir() -> &'static str {
    if cfg!(any(debug_assertions, feature = "e2e")) {
        concat!(env!("OUT_DIR"), "/locale")
    } else {
        "/usr/share/locale"
    }
}

fn init_gettext() {
    // The locale itself is set by QGuiApplication (setlocale(LC_ALL, "")),
    // before any message is looked up.
    let _ = gettextrs::bindtextdomain(GETTEXT_DOMAIN, locale_dir());
    let _ = gettextrs::bind_textdomain_codeset(GETTEXT_DOMAIN, "UTF-8");
    let _ = gettextrs::textdomain(GETTEXT_DOMAIN);
}
