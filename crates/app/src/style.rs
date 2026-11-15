//! Loads the stylesheet. Debug builds read `data/style.css` from disk and
//! reload it whenever it changes, so CSS edits show up without a restart.

use gtk::{gdk, gio};

const RESOURCE: &str = "/dev/cairn/Files/css/style.css";

pub fn load() {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();

    #[cfg(debug_assertions)]
    let loaded_from_disk = live::load_and_watch(&provider);
    #[cfg(not(debug_assertions))]
    let loaded_from_disk = false;

    if !loaded_from_disk {
        provider.load_from_resource(RESOURCE);
    }
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

#[cfg(debug_assertions)]
mod live {
    use std::cell::RefCell;
    use std::path::PathBuf;

    use gtk::prelude::*;

    use super::gio;

    thread_local! {
        // The monitor must outlive this function, or it stops reporting changes.
        static MONITOR: RefCell<Option<gio::FileMonitor>> = const { RefCell::new(None) };
    }

    fn source_path() -> PathBuf {
        if let Some(dir) = std::env::var_os("CAIRN_DATA_DIR") {
            return PathBuf::from(dir).join("style.css");
        }
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/style.css"))
    }

    /// Returns false when the source file is not available (for example a
    /// debug binary copied elsewhere), so the caller falls back to the resource.
    pub fn load_and_watch(provider: &gtk::CssProvider) -> bool {
        let path = source_path();
        if !path.is_file() {
            return false;
        }
        provider.load_from_path(&path);

        let file = gio::File::for_path(&path);
        let Ok(monitor) = file.monitor_file(gio::FileMonitorFlags::NONE, gio::Cancellable::NONE)
        else {
            return true;
        };
        let provider = provider.clone();
        monitor.connect_changed(move |_, _, _, event| {
            if matches!(
                event,
                gio::FileMonitorEvent::ChangesDoneHint | gio::FileMonitorEvent::Created
            ) {
                provider.load_from_path(&path);
            }
        });
        MONITOR.with(|slot| *slot.borrow_mut() = Some(monitor));
        true
    }
}
