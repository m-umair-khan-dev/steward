//! The user's `app.db`, shared by every window on the UI thread.

use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

use cairn_core::store::Store;

thread_local! {
    static STORE: OnceCell<Rc<RefCell<Store>>> = const { OnceCell::new() };
}

/// The open store. If `app.db` cannot be opened, an in-memory store is used so
/// the app still works (choices then last until it quits).
pub fn store() -> Rc<RefCell<Store>> {
    STORE.with(|cell| {
        cell.get_or_init(|| {
            let path = Store::default_path();
            let store = Store::open(&path).unwrap_or_else(|error| {
                glib::g_warning!("cairn", "could not open {}: {error}", path.display());
                Store::open_in_memory().expect("in-memory store")
            });
            Rc::new(RefCell::new(store))
        })
        .clone()
    })
}

/// Seconds since the epoch.
pub fn now() -> i64 {
    glib::real_time() / 1_000_000
}
