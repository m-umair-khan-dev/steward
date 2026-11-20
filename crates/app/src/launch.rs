//! What to do at launch (P6-T06): the command line and the desktop file's
//! actions. Cairn runs once per session; a second launch hands its request
//! to the running window over D-Bus (org.gtk.Actions) and exits.
//!
//!     cairn [FOLDER-OR-FILE…]      open a folder (a file: its folder, selected)
//!     cairn --new-window           bring the window forward at the Home folder
//!     cairn --new-workspace        ask for a new workspace's name
//!     cairn --free-up-space        open Free Up Space

use std::cell::RefCell;

use gio::prelude::*;

use cairn_core::{APP_ID, APP_OBJECT_PATH};

/// One action to run: ("win" or "app", name, string parameter or none).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub scope: &'static str,
    pub name: &'static str,
    pub param: Option<String>,
}

fn win(name: &'static str, param: Option<String>) -> Request {
    Request {
        scope: "win",
        name,
        param,
    }
}

/// The requests in a command line (without the program name). Unknown
/// options are ignored; paths are made absolute.
pub fn parse(args: &[String]) -> Vec<Request> {
    let mut out = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--new-window" => out.push(win(
                "go-to",
                Some(glib::home_dir().display().to_string()),
            )),
            "--new-workspace" => out.push(win("workspace-new", None)),
            "--free-up-space" => out.push(win("tool-open", Some("clean".into()))),
            other if other.starts_with("--") => {}
            path => {
                let file = gio::File::for_commandline_arg(path);
                let target = file.path().map(|p| p.display().to_string()).unwrap_or_else(|| file.uri().to_string());
                out.push(win("go-to", Some(target)));
            }
        }
    }
    out
}

thread_local! {
    static STARTUP: RefCell<Vec<Request>> = const { RefCell::new(Vec::new()) };
}

/// Sends `requests` to a Cairn that is already running. Returns false when
/// none is running (this process then becomes Cairn).
pub fn forward(requests: &[Request]) -> bool {
    let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) else {
        return false;
    };
    let owned = bus
        .call_sync(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "NameHasOwner",
            Some(&(APP_ID,).to_variant()),
            Some(glib::VariantTy::new("(b)").expect("a valid type")),
            gio::DBusCallFlags::NONE,
            2000,
            gio::Cancellable::NONE,
        )
        .ok()
        .and_then(|reply| reply.child_value(0).get::<bool>())
        .unwrap_or(false);
    if !owned {
        return false;
    }
    for request in requests {
        let path = if request.scope == "app" {
            APP_OBJECT_PATH.to_string()
        } else {
            format!("{APP_OBJECT_PATH}/window/1")
        };
        let params: Vec<glib::Variant> = request
            .param
            .iter()
            .map(|p| p.to_variant().to_variant())
            .collect();
        let platform = glib::VariantDict::new(None).end();
        let args = glib::Variant::tuple_from_iter([
            request.name.to_variant(),
            glib::Variant::array_from_iter_with_type(glib::VariantTy::VARIANT, params),
            platform,
        ]);
        if let Err(error) = bus.call_sync(
            Some(APP_ID),
            &path,
            "org.gtk.Actions",
            "Activate",
            Some(&args),
            None,
            gio::DBusCallFlags::NONE,
            5000,
            gio::Cancellable::NONE,
        ) {
            eprintln!("cairn: could not pass {} to the running Cairn: {error}", request.name);
        }
    }
    true
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_and_paths() {
        let args: Vec<String> = ["--free-up-space", "--bogus", "/tmp"].map(String::from).to_vec();
        let requests = parse(&args);
        assert_eq!(requests[0], win("tool-open", Some("clean".into())));
        assert_eq!(requests[1], win("go-to", Some("/tmp".into())));
        assert_eq!(requests.len(), 2);
        assert_eq!(parse(&["--new-workspace".into()])[0].name, "workspace-new");
    }
}
