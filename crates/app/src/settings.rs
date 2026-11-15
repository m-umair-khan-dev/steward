//! Access to the app's GSettings (schema in `data/dev.cairn.Files.gschema.xml`).
//!
//! Development and test builds load the schema compiled by build.rs into
//! OUT_DIR, so nothing needs installing. Release builds use the installed schema.

use std::cell::OnceCell;

use gtk::gio;
use gtk::prelude::*;

use cairn_core::APP_ID;

thread_local! {
    static SETTINGS: OnceCell<gio::Settings> = const { OnceCell::new() };
}

/// The shared settings object for this thread (the UI thread).
pub fn get() -> gio::Settings {
    SETTINGS.with(|cell| cell.get_or_init(load).clone())
}

fn load() -> gio::Settings {
    #[cfg(any(debug_assertions, feature = "e2e"))]
    {
        let dir = concat!(env!("OUT_DIR"), "/schemas");
        let parent = gio::SettingsSchemaSource::default();
        if let Ok(source) = gio::SettingsSchemaSource::from_directory(dir, parent.as_ref(), false) {
            if let Some(schema) = source.lookup(APP_ID, false) {
                return gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None);
            }
        }
    }
    gio::Settings::new(APP_ID)
}

/// Applies the colour-scheme setting and follows changes to it.
pub fn follow_color_scheme() {
    let settings = get();
    let apply = |settings: &gio::Settings| {
        let scheme = match settings.string("color-scheme").as_str() {
            "light" => adw::ColorScheme::ForceLight,
            "dark" => adw::ColorScheme::ForceDark,
            _ => adw::ColorScheme::Default,
        };
        adw::StyleManager::default().set_color_scheme(scheme);
    };
    apply(&settings);
    settings.connect_changed(Some("color-scheme"), move |settings, _| apply(settings));
}
