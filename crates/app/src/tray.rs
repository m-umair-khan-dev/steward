//! The tray icon (P8-T03; DESIGN.md §7.3): a StatusNotifierItem in the top
//! bar. Clicking it opens or closes the Shelf; a middle click brings the
//! main window forward. Off by default.
//!
//! The item has a bus name of its own, so turning it off removes it from the
//! panel (the watcher sees the name go). The watcher is the panel's: on
//! Ubuntu, the AppIndicator extension. Without one there is no icon
//! (ADR 0014); when one appears later, the item registers then.

use std::cell::RefCell;

use gio::prelude::*;

use crate::{actions, controller};

const ITEM_PATH: &str = "/StatusNotifierItem";
const WATCHER: &str = "org.kde.StatusNotifierWatcher";
const XML: &str = r#"
<node>
  <interface name="org.kde.StatusNotifierItem">
    <property name="Category" type="s" access="read"/>
    <property name="Id" type="s" access="read"/>
    <property name="Title" type="s" access="read"/>
    <property name="Status" type="s" access="read"/>
    <property name="IconName" type="s" access="read"/>
    <property name="IconThemePath" type="s" access="read"/>
    <property name="ToolTip" type="(sa(iiay)ss)" access="read"/>
    <property name="ItemIsMenu" type="b" access="read"/>
    <property name="Menu" type="o" access="read"/>
    <method name="Activate"><arg type="i" name="x" direction="in"/><arg type="i" name="y" direction="in"/></method>
    <method name="SecondaryActivate"><arg type="i" name="x" direction="in"/><arg type="i" name="y" direction="in"/></method>
    <method name="ContextMenu"><arg type="i" name="x" direction="in"/><arg type="i" name="y" direction="in"/></method>
    <method name="Scroll"><arg type="i" name="delta" direction="in"/><arg type="s" name="orientation" direction="in"/></method>
    <signal name="NewIcon"/>
    <signal name="NewStatus"><arg type="s" name="status"/></signal>
  </interface>
</node>"#;

struct Tray {
    owner: gio::OwnerId,
    watch: gio::WatcherId,
    registration: Option<gio::RegistrationId>,
    name: String,
}

thread_local! {
    static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) };
}

/// Our icons: installed builds use the theme; development and test builds
/// point at the repository's icons.
fn icon_theme_path() -> String {
    if cfg!(any(debug_assertions, feature = "e2e")) {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/icons").to_string()
    } else {
        String::new()
    }
}

fn property(name: &str) -> glib::Variant {
    match name {
        "Category" => "ApplicationStatus".to_variant(),
        "Id" => steward_core::APP_ID.to_variant(),
        "Title" => "Steward".to_variant(),
        "Status" => "Active".to_variant(),
        "IconName" => format!("{}-symbolic", steward_core::APP_ID).to_variant(),
        "IconThemePath" => icon_theme_path().to_variant(),
        "ToolTip" => {
            let pixmaps: Vec<(i32, i32, Vec<u8>)> = Vec::new();
            (
                String::new(),
                pixmaps,
                "Steward".to_string(),
                gettextrs::gettext("Click to open the Shelf"),
            )
                .to_variant()
        }
        "ItemIsMenu" => false.to_variant(),
        _ => glib::variant::ObjectPath::try_from("/NO_DBUSMENU".to_string())
            .expect("a valid object path")
            .to_variant(),
    }
}

fn register_with_watcher(connection: &gio::DBusConnection, name: &str) {
    connection.call(
        Some(WATCHER),
        "/StatusNotifierWatcher",
        WATCHER,
        "RegisterStatusNotifierItem",
        Some(&(name,).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        5000,
        gio::Cancellable::NONE,
        |result| {
            if let Err(error) = result {
                glib::g_warning!("steward", "the panel did not take the tray icon: {error}");
            }
        },
    );
}

/// Shows or removes the tray icon.
pub fn show(enabled: bool) {
    if !enabled {
        if let Some(tray) = TRAY.with(|t| t.borrow_mut().take()) {
            if let (Some(registration), Ok(bus)) = (
                tray.registration,
                gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE),
            ) {
                let _ = bus.unregister_object(registration);
            }
            gio::bus_unwatch_name(tray.watch);
            gio::bus_unown_name(tray.owner);
        }
        return;
    }
    if TRAY.with(|t| t.borrow().is_some()) {
        return;
    }
    let name = format!("org.kde.StatusNotifierItem-{}-1", std::process::id());
    let owner = gio::bus_own_name(
        gio::BusType::Session,
        &name,
        gio::BusNameOwnerFlags::NONE,
        |connection, _| {
            let info = gio::DBusNodeInfo::for_xml(XML)
                .expect("valid introspection XML")
                .lookup_interface("org.kde.StatusNotifierItem")
                .expect("interface in XML");
            let registration = connection
                .register_object(ITEM_PATH, &info)
                .method_call(|_, _, _, _, method, _, invocation| {
                    invocation.return_value(None);
                    match method {
                        "Activate" => {
                            glib::idle_add_local_once(|| actions::activate("app.shelf", ""))
                        }
                        "SecondaryActivate" => glib::idle_add_local_once(|| {
                            controller::with(|c| c.present());
                        }),
                        _ => return,
                    };
                })
                .property(|_, _, _, _, name| property(name))
                .build();
            match registration {
                Ok(id) => TRAY.with(|t| {
                    if let Some(tray) = t.borrow_mut().as_mut() {
                        tray.registration = Some(id);
                    }
                }),
                Err(error) => glib::g_warning!("steward", "could not offer the tray icon: {error}"),
            }
        },
        |connection, name| {
            // Our name is ours: tell the panel, if it is there already.
            if TRAY.with(|t| t.borrow().is_some()) {
                register_with_watcher(&connection, name);
            }
        },
        |_, _| {},
    );
    // The panel may start (or restart) after us.
    let item = name.clone();
    let watch = gio::bus_watch_name(
        gio::BusType::Session,
        WATCHER,
        gio::BusNameWatcherFlags::NONE,
        move |connection, _, _| register_with_watcher(&connection, &item),
        |_, _| {},
    );
    TRAY.with(|t| {
        *t.borrow_mut() = Some(Tray {
            owner,
            watch,
            registration: None,
            name,
        })
    });
}

/// The item's bus name, while the icon is shown (tests read it).
#[cfg_attr(not(feature = "e2e"), allow(dead_code))]
pub fn item_name() -> Option<String> {
    TRAY.with(|t| t.borrow().as_ref().map(|tray| tray.name.clone()))
}
