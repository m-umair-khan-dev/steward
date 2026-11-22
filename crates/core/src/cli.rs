//! The `cairn` command line (P6-T06, P7-T10; DESIGN.md §4.2): what each
//! command asks the window to do, and handing that to a running Cairn over
//! D-Bus (`org.gtk.Actions`). Shared by the app binary and `cairn-cli`.
//!
//!     cairn [FOLDER-OR-FILE…]          open a folder (a file: its folder, selected)
//!     cairn open PATH…                 the same, as a command
//!     cairn open-workspace NAME        open a workspace by name (or #1…#9)
//!     cairn new-workspace [NAME]       make a workspace (no name: ask for one)
//!     cairn add-link URL               save a link to Bookmarks
//!     cairn free-up-space              open Free Up Space
//!     cairn shelf                      open or close the Shelf
//!     cairn --new-window | --new-workspace | --add-link | --free-up-space
//!                                      (the desktop file's actions)

use gio::prelude::*;

use crate::{APP_ID, APP_OBJECT_PATH};

/// One action to run in the window: ("win" or "app", name, parameter).
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Parsed {
    Run(Vec<Request>),
    Help,
    Version,
    /// A mistake on the command line; the message says what.
    Wrong(String),
}

pub const USAGE: &str = "Usage:
  cairn [FOLDER-OR-FILE…]        Open a folder, or a file's folder with it selected
  cairn open PATH…               The same
  cairn open-workspace NAME      Open a workspace by its name (or #1 to #9)
  cairn new-workspace [NAME]     Make a workspace (without a name, ask for one)
  cairn add-link URL             Save a link to Bookmarks
  cairn free-up-space            Open Free Up Space
  cairn shelf                    Open or close the Shelf
  cairn --help | --version";

/// Reads a command line (without the program name). `absolute` turns a
/// path argument into an absolute path or URI.
pub fn parse(args: &[String], absolute: &dyn Fn(&str) -> String) -> Parsed {
    let rest = |n: usize| args.get(n..).unwrap_or_default();
    let one = |what: &str| -> Result<String, String> {
        match rest(1) {
            [value] if !value.trim().is_empty() => Ok(value.clone()),
            [] => Err(format!("{} needs {what}", args[0])),
            _ => Err(format!(
                "{} takes one {what} (use quotes around a name with spaces)",
                args[0]
            )),
        }
    };
    let Some(first) = args.first() else {
        return Parsed::Run(Vec::new());
    };
    let result = match first.as_str() {
        "help" | "--help" | "-h" => return Parsed::Help,
        "--version" | "-V" => return Parsed::Version,
        "open" => {
            if rest(1).is_empty() {
                Err("open needs a path".to_string())
            } else {
                Ok(rest(1)
                    .iter()
                    .map(|p| win("go-to", Some(absolute(p))))
                    .collect())
            }
        }
        "open-workspace" => {
            one("a workspace name").map(|name| vec![win("workspace-open", Some(name))])
        }
        "new-workspace" => match rest(1) {
            [] => Ok(vec![win("workspace-new", None)]),
            [name] => Ok(vec![win("workspace-create", Some(name.clone()))]),
            _ => Err("new-workspace takes one name (use quotes around a name with spaces)".into()),
        },
        "add-link" => one("a link").map(|url| vec![win("bookmark-add", Some(url))]),
        "free-up-space" => Ok(vec![win("tool-open", Some("clean".into()))]),
        "shelf" => Ok(vec![Request {
            scope: "app",
            name: "shelf",
            param: None,
        }]),
        _ => {
            // Desktop-file options and plain paths (P6-T06).
            let mut out = Vec::new();
            for arg in args {
                match arg.as_str() {
                    "--new-window" => {
                        out.push(win("go-to", Some(glib::home_dir().display().to_string())))
                    }
                    "--new-workspace" => out.push(win("workspace-new", None)),
                    "--free-up-space" => out.push(win("tool-open", Some("clean".into()))),
                    "--add-link" => out.push(win("bookmark-add-ask", None)),
                    other if other.starts_with("--") => {}
                    path => out.push(win("go-to", Some(absolute(path)))),
                }
            }
            Ok(out)
        }
    };
    match result {
        Ok(requests) => Parsed::Run(requests),
        Err(message) => Parsed::Wrong(message),
    }
}

/// A path argument made absolute (relative to the current folder), or the
/// URI for a non-local location.
pub fn absolute(arg: &str) -> String {
    let file = gio::File::for_commandline_arg(arg);
    file.path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| file.uri().to_string())
}

/// True when a Cairn is running in this session.
pub fn running(bus: &gio::DBusConnection) -> bool {
    bus.call_sync(
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
    .unwrap_or(false)
}

/// Sends `requests` to the running Cairn. Returns false when none is
/// running; errors are printed with `program` as the prefix.
pub fn forward(requests: &[Request], program: &str) -> bool {
    let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) else {
        return false;
    };
    if !running(&bus) {
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
            eprintln!(
                "{program}: could not pass {} to the running Cairn: {error}",
                request.name
            );
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Parsed {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        parse(&args, &|p| format!("/abs/{p}"))
    }

    #[test]
    fn commands() {
        assert_eq!(
            run(&["open", "a", "b"]),
            Parsed::Run(vec![
                win("go-to", Some("/abs/a".into())),
                win("go-to", Some("/abs/b".into()))
            ])
        );
        assert_eq!(
            run(&["open-workspace", "Taxes 2026"]),
            Parsed::Run(vec![win("workspace-open", Some("Taxes 2026".into()))])
        );
        assert_eq!(
            run(&["new-workspace"]),
            Parsed::Run(vec![win("workspace-new", None)])
        );
        assert_eq!(
            run(&["new-workspace", "Trip"]),
            Parsed::Run(vec![win("workspace-create", Some("Trip".into()))])
        );
        assert_eq!(
            run(&["add-link", "example.com"]),
            Parsed::Run(vec![win("bookmark-add", Some("example.com".into()))])
        );
        assert_eq!(
            run(&["free-up-space"]),
            Parsed::Run(vec![win("tool-open", Some("clean".into()))])
        );
        assert_eq!(run(&["--help"]), Parsed::Help);
        assert!(matches!(run(&["add-link"]), Parsed::Wrong(_)));
        assert!(matches!(
            run(&["open-workspace", "Taxes", "2026"]),
            Parsed::Wrong(_)
        ));
    }

    #[test]
    fn desktop_options_and_paths() {
        let Parsed::Run(requests) = run(&["--free-up-space", "--bogus", "docs"]) else {
            panic!()
        };
        assert_eq!(
            requests,
            vec![
                win("tool-open", Some("clean".into())),
                win("go-to", Some("/abs/docs".into()))
            ]
        );
        assert_eq!(run(&[]), Parsed::Run(vec![]));
    }
}
