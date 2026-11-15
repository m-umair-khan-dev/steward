//! Application set-up: resources, actions, shortcuts and the main window.

use std::process::ExitCode;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::gio;

use cairn_core::{APP_ID, APP_OBJECT_PATH};

use crate::style;
use crate::window::CairnWindow;

/// The gettext domain. Translations are wired up in Phase 6.
const GETTEXT_DOMAIN: &str = "cairn";

pub fn run() -> ExitCode {
    init_gettext();
    gio::resources_register_include!("cairn.gresource").expect("embedded GResource is valid");

    let app = adw::Application::builder()
        .application_id(APP_ID)
        .resource_base_path(APP_OBJECT_PATH)
        .build();

    app.connect_startup(|app| {
        style::load();
        add_actions(app);
        #[cfg(feature = "e2e")]
        crate::test_iface::register(app);
    });
    app.connect_activate(|app| {
        let window = app
            .active_window()
            .unwrap_or_else(|| CairnWindow::new(app).upcast());
        window.present();
    });

    let code = app.run();
    ExitCode::from(code.get())
}

fn init_gettext() {
    // GTK calls setlocale() itself during start-up, so only the domain is set here.
    // Failing to set up translations must never stop the app; it falls back to English.
    let _ = gettextrs::bindtextdomain(GETTEXT_DOMAIN, "/usr/share/locale");
    let _ = gettextrs::bind_textdomain_codeset(GETTEXT_DOMAIN, "UTF-8");
    let _ = gettextrs::textdomain(GETTEXT_DOMAIN);
}

fn add_actions(app: &adw::Application) {
    let quit = gio::ActionEntry::builder("quit")
        .activate(|app: &adw::Application, _, _| app.quit())
        .build();
    let about = gio::ActionEntry::builder("about")
        .activate(|app: &adw::Application, _, _| show_about(app))
        .build();
    app.add_action_entries([quit, about]);
    app.set_accels_for_action("app.quit", &["<Control>q"]);
}

fn show_about(app: &adw::Application) {
    let dialog = adw::AboutDialog::builder()
        .application_name(gettext("Cairn"))
        .application_icon(APP_ID)
        .version(env!("CARGO_PKG_VERSION"))
        .developer_name(gettext("Cairn contributors"))
        .license_type(gtk::License::Gpl30)
        .build();
    let parent = app.active_window();
    dialog.present(parent.as_ref());
}
