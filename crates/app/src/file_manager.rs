//! "Use Steward as the default file manager" (P6-T07), off unless chosen in
//! Preferences. When on:
//!
//! - Steward owns `org.freedesktop.FileManager1` while it runs, so other apps'
//!   "Show in folder" (ShowItems), "Open folder" (ShowFolders) and
//!   "Properties" (ShowItemProperties) come here. Files (Nautilus) normally
//!   owns the name; it gives it up only when it is not running, so this
//!   works when Files is closed.
//! - Steward becomes the default app for folders (`inode/directory`).
//!
//! Turning it off releases the name and gives folders back to the system's
//! default.

use std::cell::RefCell;

use gio::prelude::*;

use crate::actions;

const NAME: &str = "org.freedesktop.FileManager1";
const PATH: &str = "/org/freedesktop/FileManager1";
const XML: &str = r#"
<node>
  <interface name="org.freedesktop.FileManager1">
    <method name="ShowFolders">
      <arg type="as" name="URIs" direction="in"/><arg type="s" name="StartupId" direction="in"/>
    </method>
    <method name="ShowItems">
      <arg type="as" name="URIs" direction="in"/><arg type="s" name="StartupId" direction="in"/>
    </method>
    <method name="ShowItemProperties">
      <arg type="as" name="URIs" direction="in"/><arg type="s" name="StartupId" direction="in"/>
    </method>
  </interface>
</node>"#;

thread_local! {
    static OWNER: RefCell<Option<(gio::OwnerId, Option<gio::RegistrationId>)>> = const { RefCell::new(None) };
}

fn show(method: &str, uris: &[String]) {
    let Some(first) = uris.first().cloned() else {
        return;
    };
    // Steward has one window: the first item is shown (as Files does with one).
    match method {
        "ShowFolders" => actions::activate("win.go-to", &first),
        "ShowItems" => actions::activate("win.go-to", &first),
        _ => {
            actions::activate("win.go-to", &first);
            if !crate::settings::get().boolean("preview-visible") {
                actions::activate("win.toggle-preview", "");
            }
        }
    }
}

/// Takes or releases the FileManager1 name.
pub fn serve(enabled: bool) {
    if !enabled {
        if let Some((owner, registration)) = OWNER.with(|o| o.borrow_mut().take()) {
            if let (Some(registration), Ok(bus)) = (
                registration,
                gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE),
            ) {
                let _ = bus.unregister_object(registration);
            }
            gio::bus_unown_name(owner);
        }
        return;
    }
    if OWNER.with(|o| o.borrow().is_some()) {
        return;
    }
    let owner = gio::bus_own_name(
        gio::BusType::Session,
        NAME,
        gio::BusNameOwnerFlags::ALLOW_REPLACEMENT,
        |connection, _| {
            let info = gio::DBusNodeInfo::for_xml(XML)
                .expect("valid introspection XML")
                .lookup_interface(NAME)
                .expect("interface in XML");
            let registration = connection
                .register_object(PATH, &info)
                .method_call(|_, _, _, _, method, params, invocation| {
                    let uris: Vec<String> = params.child_get(0);
                    let method = method.to_string();
                    invocation.return_value(None);
                    glib::idle_add_local_once(move || show(&method, &uris));
                })
                .build();
            match registration {
                Ok(id) => OWNER.with(|o| {
                    if let Some(entry) = o.borrow_mut().as_mut() {
                        entry.1 = Some(id);
                    }
                }),
                Err(error) => glib::g_warning!("steward", "could not serve {NAME}: {error}"),
            }
        },
        |_, _| {},
        |_, _| {},
    );
    OWNER.with(|o| *o.borrow_mut() = Some((owner, None)));
}

/// Makes Steward the default app for folders, or gives them back. Returns
/// false when Steward's desktop file is not installed (a development build).
pub fn set_default_for_folders(enabled: bool) -> bool {
    let id = format!("{}.desktop", steward_core::APP_ID);
    let Some(app) = gio::AppInfo::all()
        .into_iter()
        .find(|a| a.id().as_deref() == Some(id.as_str()))
    else {
        return false;
    };
    if enabled {
        app.set_as_default_for_type("inode/directory").is_ok()
    } else {
        // Only undo our own choice: another app chosen since stays chosen.
        let ours = gio::AppInfo::default_for_type("inode/directory", false)
            .is_some_and(|d| d.id() == app.id());
        if ours {
            gio::AppInfo::reset_type_associations("inode/directory");
        }
        true
    }
}
