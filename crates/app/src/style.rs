//! Stylesheets (ADR 0008). Four layers:
//!
//! - `style.css`: layout and the light palette, always loaded;
//! - `style-dark.css`: the dark palette, loaded while the style manager is dark;
//! - `accent.css` / `accent-dark.css`: Cairn's purple accent, loaded unless the
//!   user chose the system accent colour (setting `accent`).
//!
//! Debug builds read the files from `data/` and reload them when they change.

use std::cell::RefCell;

use gtk::prelude::*;
use gtk::{gdk, gio};

use crate::settings;

const BASE: u32 = gtk::STYLE_PROVIDER_PRIORITY_APPLICATION;

thread_local! {
    static PROVIDERS: RefCell<Vec<(&'static str, gtk::CssProvider, bool)>> = const { RefCell::new(Vec::new()) };
    static MONITORS: RefCell<Vec<gio::FileMonitor>> = const { RefCell::new(Vec::new()) };
}

pub fn load() {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    for name in ["style.css", "style-dark.css", "accent.css", "accent-dark.css"] {
        let provider = gtk::CssProvider::new();
        load_file(&provider, name);
        PROVIDERS.with(|p| p.borrow_mut().push((name, provider, false)));
    }
    let apply = move || refresh(&display);
    let manager = adw::StyleManager::default();
    let apply_dark = apply.clone();
    manager.connect_dark_notify(move |_| apply_dark());
    let apply_setting = apply.clone();
    settings::get().connect_changed(Some("accent"), move |_, _| apply_setting());
    apply();
}

/// Adds or removes the dark and accent layers to match the current state.
fn refresh(display: &gdk::Display) {
    let dark = adw::StyleManager::default().is_dark();
    let cairn_accent = settings::get().string("accent").as_str() != "system";
    PROVIDERS.with(|providers| {
        for (name, provider, attached) in providers.borrow_mut().iter_mut() {
            let (wanted, priority) = match *name {
                "style.css" => (true, BASE),
                "style-dark.css" => (dark, BASE + 1),
                "accent.css" => (cairn_accent, BASE + 2),
                _ => (cairn_accent && dark, BASE + 3),
            };
            if wanted && !*attached {
                gtk::style_context_add_provider_for_display(display, provider, priority);
            } else if !wanted && *attached {
                gtk::style_context_remove_provider_for_display(display, provider);
            }
            *attached = wanted;
        }
    });
}

fn load_file(provider: &gtk::CssProvider, name: &str) {
    #[cfg(debug_assertions)]
    {
        let dir = std::env::var_os("CAIRN_DATA_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| concat!(env!("CARGO_MANIFEST_DIR"), "/../../data").into());
        let path = dir.join(name);
        if path.is_file() {
            provider.load_from_path(&path);
            if let Ok(monitor) =
                gio::File::for_path(&path).monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE)
            {
                let provider = provider.clone();
                monitor.connect_changed(move |_, _, _, event| {
                    if matches!(event, gio::FileMonitorEvent::ChangesDoneHint | gio::FileMonitorEvent::Created) {
                        provider.load_from_path(&path);
                    }
                });
                MONITORS.with(|m| m.borrow_mut().push(monitor));
            }
            return;
        }
    }
    provider.load_from_resource(&format!("/dev/cairn/Files/css/{name}"));
}
