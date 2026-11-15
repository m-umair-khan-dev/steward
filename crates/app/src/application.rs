//! Application set-up: resources, actions, shortcuts and the main window.

use std::process::ExitCode;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::gio;

use cairn_core::{APP_ID, APP_OBJECT_PATH};

use crate::window::CairnWindow;
use crate::{preferences, settings, style};

/// The gettext domain. Translations are wired up in Phase 6.
const GETTEXT_DOMAIN: &str = "cairn";

/// Keyboard shortcuts (DESIGN.md Appendix A). The shortcuts window
/// (data/ui/help-overlay.blp) shows these; keep both in step.
pub const ACCELS: &[(&str, &[&str])] = &[
    ("app.quit", &["<Control>q"]),
    ("app.preferences", &["<Control>comma"]),
    ("win.back", &["<Alt>Left", "Back"]),
    ("win.forward", &["<Alt>Right", "Forward"]),
    ("win.up", &["<Alt>Up"]),
    ("win.home", &["<Alt>Home"]),
    ("win.reload", &["<Control>r"]),
    ("win.location", &["<Control>l"]),
    ("win.filter", &["<Control>f"]),
    ("win.open-selected", &["<Control>o"]),
    ("win.view-mode('grid')", &["<Control>1"]),
    ("win.view-mode('list')", &["<Control>2"]),
    ("win.zoom-in", &["<Control>plus", "<Control>equal", "<Control>KP_Add"]),
    ("win.zoom-out", &["<Control>minus", "<Control>KP_Subtract"]),
    ("win.zoom-reset", &["<Control>0"]),
    ("win.show-hidden", &["<Control>h"]),
    ("win.toggle-sidebar", &["F9"]),
    ("win.toggle-preview", &["<Control><Shift>p"]),
];

pub fn run() -> ExitCode {
    init_gettext();
    gio::resources_register_include!("cairn.gresource").expect("embedded GResource is valid");

    let app = adw::Application::builder()
        .application_id(APP_ID)
        .resource_base_path(APP_OBJECT_PATH)
        .build();

    app.connect_startup(|app| {
        style::load();
        settings::follow_color_scheme();
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
    app.connect_shutdown(|_| {
        // Make sure every setting reached dconf before the process ends.
        gio::Settings::sync();
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
    let prefs = gio::ActionEntry::builder("preferences")
        .activate(|app: &adw::Application, _, _| preferences::show(app.active_window().as_ref()))
        .build();
    app.add_action_entries([quit, about, prefs]);
    app.add_action(&settings::get().create_action("color-scheme"));
    for (action, accels) in ACCELS {
        app.set_accels_for_action(action, accels);
    }
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
