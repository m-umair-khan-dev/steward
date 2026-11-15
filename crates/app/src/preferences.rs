//! Preferences (DESIGN.md P1-T12): General and Appearance pages, all stored in
//! GSettings and applied at once.

use gettextrs::gettext;
use gtk::prelude::*;
use gtk::gio;

use adw::prelude::*;

use crate::settings;

/// A combo row bound to a string setting with fixed choices.
fn choice_row(settings: &gio::Settings, key: &'static str, title: &str, choices: &[(&'static str, String)]) -> adw::ComboRow {
    let labels: Vec<&str> = choices.iter().map(|(_, label)| label.as_str()).collect();
    let row = adw::ComboRow::builder().title(title).model(&gtk::StringList::new(&labels)).build();
    let ids: Vec<&'static str> = choices.iter().map(|(id, _)| *id).collect();
    let select = {
        let ids = ids.clone();
        move |row: &adw::ComboRow, settings: &gio::Settings| {
            let current = settings.string(key);
            let index = ids.iter().position(|id| *id == current.as_str()).unwrap_or(0);
            row.set_selected(index as u32);
        }
    };
    select(&row, settings);
    let s = settings.clone();
    row.connect_selected_notify(move |row| {
        if let Some(id) = ids.get(row.selected() as usize) {
            if s.string(key).as_str() != *id {
                let _ = s.set_string(key, id);
            }
        }
    });
    let weak = row.downgrade();
    settings.connect_changed(Some(key), move |settings, _| {
        if let Some(row) = weak.upgrade() {
            select(&row, settings);
        }
    });
    row
}

pub fn show(parent: Option<&gtk::Window>) {
    let settings = settings::get();
    let dialog = adw::PreferencesDialog::new();
    dialog.set_title(&gettext("Preferences"));

    let general = adw::PreferencesPage::builder().title(gettext("General")).icon_name("folder-symbolic").build();
    let folders = adw::PreferencesGroup::builder().title(gettext("Folders")).build();
    folders.add(&choice_row(
        &settings,
        "default-view",
        &gettext("Open Folders As"),
        &[("grid", gettext("Grid")), ("list", gettext("List"))],
    ));
    folders.add(&choice_row(
        &settings,
        "row-size",
        &gettext("Item Size"),
        &[("small", gettext("Small")), ("medium", gettext("Medium")), ("large", gettext("Large"))],
    ));
    let hidden = adw::SwitchRow::builder().title(gettext("Show Hidden Files")).build();
    settings.bind("show-hidden", &hidden, "active").build();
    folders.add(&hidden);
    general.add(&folders);

    let appearance = adw::PreferencesPage::builder()
        .title(gettext("Appearance"))
        .icon_name("preferences-desktop-appearance-symbolic")
        .build();
    let style = adw::PreferencesGroup::builder().title(gettext("Style")).build();
    style.add(&choice_row(
        &settings,
        "color-scheme",
        &gettext("Light or Dark"),
        &[("system", gettext("Follow System")), ("light", gettext("Light")), ("dark", gettext("Dark"))],
    ));
    let system_accent = adw::SwitchRow::builder()
        .title(gettext("Use System Accent Colour"))
        .subtitle(gettext("Instead of Cairn's purple"))
        .build();
    system_accent.set_active(settings.string("accent").as_str() == "system");
    let s = settings.clone();
    system_accent.connect_active_notify(move |row| {
        let _ = s.set_string("accent", if row.is_active() { "system" } else { "cairn" });
    });
    style.add(&system_accent);
    appearance.add(&style);

    dialog.add(&general);
    dialog.add(&appearance);
    dialog.present(parent);
}
