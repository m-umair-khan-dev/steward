//! D-Bus: the bus name dev.cairn.Files (one instance), the exported action
//! groups (org.gtk.Actions at /dev/cairn/Files and /dev/cairn/Files/window/1),
//! and, in e2e builds, the Test1 interface (DESIGN.md §10.2).

use std::cell::RefCell;
use std::collections::HashMap;

use gio::prelude::*;

use cairn_core::{APP_ID, APP_OBJECT_PATH};

use crate::actions;
#[cfg(feature = "e2e")]
use crate::controller;

thread_local! {
    static CONNECTION: RefCell<Option<gio::DBusConnection>> = const { RefCell::new(None) };
    static PENDING_SHOTS: RefCell<HashMap<String, gio::DBusMethodInvocation>> = RefCell::new(HashMap::new());
}

/// Owns the bus name and exports everything. If another instance owns the
/// name, `on_taken` runs (the caller quits).
pub fn start(on_taken: impl Fn() + 'static) {
    let acquired = std::rc::Rc::new(std::cell::Cell::new(false));
    let acquired_flag = acquired.clone();
    gio::bus_own_name(
        gio::BusType::Session,
        APP_ID,
        gio::BusNameOwnerFlags::NONE,
        |connection, _| {
            let (app, win) = actions::groups();
            if let Err(error) = connection.export_action_group(APP_OBJECT_PATH, &app) {
                glib::g_warning!("cairn", "could not export app actions: {error}");
            }
            if let Err(error) =
                connection.export_action_group(&format!("{APP_OBJECT_PATH}/window/1"), &win)
            {
                glib::g_warning!("cairn", "could not export window actions: {error}");
            }
            #[cfg(feature = "e2e")]
            register_test_interface(&connection);
            CONNECTION.with(|c| *c.borrow_mut() = Some(connection));
        },
        move |_, _| acquired_flag.set(true),
        move |_, _| {
            if !acquired.get() {
                eprintln!("cairn: another Cairn is already running");
                on_taken();
            }
        },
    );
}

/// QML finished a screenshot requested through Test1.
pub fn screenshot_done(path: &str, ok: bool) {
    if let Some(invocation) = PENDING_SHOTS.with(|p| p.borrow_mut().remove(path)) {
        if ok {
            invocation.return_value(Some(&(true,).to_variant()));
        } else {
            invocation.return_dbus_error("org.freedesktop.DBus.Error.Failed", "screenshot failed");
        }
    }
}

#[cfg(feature = "e2e")]
fn register_test_interface(connection: &gio::DBusConnection) {
    const XML: &str = r#"
<node>
  <interface name="dev.cairn.Files.Test1">
    <method name="State"><arg type="s" name="json" direction="out"/></method>
    <method name="WaitIdle">
      <arg type="u" name="timeout_ms" direction="in"/><arg type="b" name="idle" direction="out"/>
    </method>
    <method name="Screenshot">
      <arg type="s" name="path" direction="in"/><arg type="b" name="ok" direction="out"/>
    </method>
    <method name="Select">
      <arg type="as" name="names" direction="in"/><arg type="b" name="ok" direction="out"/>
    </method>
  </interface>
</node>"#;
    let info = gio::DBusNodeInfo::for_xml(XML)
        .expect("valid introspection XML")
        .lookup_interface("dev.cairn.Files.Test1")
        .expect("interface in XML");
    let result = connection
        .register_object(&format!("{APP_OBJECT_PATH}/Test1"), &info)
        .method_call(|_, _, _, _, method, params, invocation| match method {
            "State" => {
                let state = controller::with(|c| c.test_state());
                let mut value = serde_json::json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "windows": 1,
                    "accels": actions::ACCELS.iter().fold(serde_json::Map::new(), |mut map, (action, param, keys, _)| {
                        let name = if param.is_empty() { action.to_string() } else { format!("{action}('{param}')") };
                        map.insert(name, serde_json::json!(keys));
                        map
                    }),
                });
                let window = state["window"].clone();
                let dialog = state["dialog"].as_str().unwrap_or("").to_string();
                value["active_window"] = serde_json::json!({
                    "title": "Cairn",
                    "width": window["width"], "height": window["height"], "is_active": window["is_active"],
                    "dialog": if dialog.is_empty() { serde_json::Value::Null } else {
                        serde_json::json!({ "type": dialog, "application_name": "Cairn", "version": env!("CARGO_PKG_VERSION") })
                    },
                });
                if let (serde_json::Value::Object(target), serde_json::Value::Object(extra)) = (&mut value, state) {
                    for (k, v) in extra {
                        target.entry(k).or_insert(v);
                    }
                }
                invocation.return_value(Some(&(value.to_string(),).to_variant()));
            }
            "WaitIdle" => {
                glib::idle_add_local_full(glib::Priority::LOW, move || {
                    invocation.clone().return_value(Some(&(true,).to_variant()));
                    glib::ControlFlow::Break
                });
            }
            "Screenshot" => {
                let path: String = params.child_get(0);
                PENDING_SHOTS.with(|p| p.borrow_mut().insert(path.clone(), invocation));
                controller::with(|c| c.request_screenshot(&path));
            }
            "Select" => {
                let names: Vec<String> = params.child_get(0);
                controller::with(|c| c.select_names(&names));
                invocation.return_value(Some(&(true,).to_variant()));
            }
            _ => invocation.return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", method),
        })
        .build();
    if let Err(error) = result {
        glib::g_warning!("cairn", "could not register Test1: {error}");
    }
}
