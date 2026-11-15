//! `dev.cairn.Files.Test1`: a D-Bus interface for end-to-end tests
//! (DESIGN.md §10.2). Compiled only with the `e2e` feature; never shipped.
//!
//! - `State() -> s`: JSON describing what the app shows.
//! - `WaitIdle(u timeout_ms) -> b`: returns once the app has no pending work.
//! - `Screenshot(s path) -> b`: renders the active window to a PNG file.

use adw::prelude::*;
use gtk::{gio, glib, gsk};
use serde_json::{Value, json};

use cairn_core::APP_OBJECT_PATH;

const INTERFACE: &str = "dev.cairn.Files.Test1";
const XML: &str = r#"
<node>
  <interface name="dev.cairn.Files.Test1">
    <method name="State">
      <arg type="s" name="json" direction="out"/>
    </method>
    <method name="WaitIdle">
      <arg type="u" name="timeout_ms" direction="in"/>
      <arg type="b" name="idle" direction="out"/>
    </method>
    <method name="Screenshot">
      <arg type="s" name="path" direction="in"/>
      <arg type="b" name="ok" direction="out"/>
    </method>
  </interface>
</node>"#;

pub fn register(app: &adw::Application) {
    let Some(connection) = app.dbus_connection() else {
        eprintln!("cairn: e2e build without a D-Bus connection; Test1 is unavailable");
        return;
    };
    let info = gio::DBusNodeInfo::for_xml(XML)
        .expect("valid introspection XML")
        .lookup_interface(INTERFACE)
        .expect("interface present in XML");
    let weak_app = app.downgrade();
    let registration = connection
        .register_object(&format!("{APP_OBJECT_PATH}/Test1"), &info)
        .method_call(
            move |_conn, _sender, _path, _iface, method, params, invocation| {
                let Some(app) = weak_app.upgrade() else {
                    invocation.return_dbus_error("org.freedesktop.DBus.Error.Failed", "app gone");
                    return;
                };
                match method {
                    "State" => {
                        invocation.return_value(Some(&(state(&app).to_string(),).to_variant()));
                    }
                    "WaitIdle" => {
                        // Phase 0 has no background work: idle means the main loop has
                        // drained everything queued before this call.
                        let _ = params;
                        glib::idle_add_local_once(move || {
                            invocation.return_value(Some(&(true,).to_variant()));
                        });
                    }
                    "Screenshot" => {
                        let path: String = params.child_get::<String>(0);
                        match screenshot(&app, &path) {
                            Ok(()) => invocation.return_value(Some(&(true,).to_variant())),
                            Err(message) => invocation
                                .return_dbus_error("org.freedesktop.DBus.Error.Failed", &message),
                        }
                    }
                    _ => invocation.return_dbus_error(
                        "org.freedesktop.DBus.Error.UnknownMethod",
                        &format!("no method {method}"),
                    ),
                }
            },
        )
        .build();
    if let Err(error) = registration {
        eprintln!("cairn: could not register {INTERFACE}: {error}");
    }
}

fn state(app: &adw::Application) -> Value {
    let windows = app.windows();
    let active = app.active_window().or_else(|| windows.first().cloned());
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "windows": windows.len(),
        "active_window": active.map(|window| window_state(&window)),
    })
}

fn window_state(window: &gtk::Window) -> Value {
    let dialog = window
        .downcast_ref::<adw::ApplicationWindow>()
        .and_then(|w| w.visible_dialog())
        .map(|dialog| dialog_state(&dialog));
    json!({
        "title": window.title().map(|t| t.to_string()),
        "width": window.width(),
        "height": window.height(),
        "is_active": window.is_active(),
        "dialog": dialog,
    })
}

fn dialog_state(dialog: &adw::Dialog) -> Value {
    let mut value = json!({
        "type": dialog.type_().name(),
        "title": dialog.title().to_string(),
    });
    if let Some(about) = dialog.downcast_ref::<adw::AboutDialog>() {
        value["application_name"] = json!(about.application_name().to_string());
        value["version"] = json!(about.version().to_string());
    }
    value
}

fn screenshot(app: &adw::Application, path: &str) -> Result<(), String> {
    let window = app
        .active_window()
        .or_else(|| app.windows().first().cloned())
        .ok_or("no window")?;
    let (width, height) = (window.width(), window.height());
    if width <= 0 || height <= 0 {
        return Err("window has no size yet".into());
    }
    let paintable = gtk::WidgetPaintable::new(Some(&window));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, f64::from(width), f64::from(height));
    let node = snapshot.to_node().ok_or("window rendered nothing")?;
    let renderer: gsk::Renderer = window.renderer().ok_or("window has no renderer")?;
    let texture = renderer.render_texture(&node, None);
    texture.save_to_png(path).map_err(|e| e.to_string())
}
