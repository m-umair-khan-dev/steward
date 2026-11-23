//! What to do at launch (P6-T06, P7-T10): the command line and the desktop
//! file's actions. Steward runs once per session; a second launch hands its
//! request to the running window over D-Bus (org.gtk.Actions) and exits.
//! The commands themselves are in `steward_core::cli`.

use std::cell::RefCell;

pub use steward_core::cli::Request;

thread_local! {
    static STARTUP: RefCell<Vec<Request>> = const { RefCell::new(Vec::new()) };
}

/// Remembers what to do once the window is up.
pub fn set_startup(requests: Vec<Request>) {
    STARTUP.with(|s| *s.borrow_mut() = requests);
}

/// Runs the remembered requests (after the controller has started).
pub fn run_startup() {
    let requests = STARTUP.with(|s| std::mem::take(&mut *s.borrow_mut()));
    for request in requests {
        crate::actions::activate(
            &format!("{}.{}", request.scope, request.name),
            request.param.as_deref().unwrap_or(""),
        );
    }
}
