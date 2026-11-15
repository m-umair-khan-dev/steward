//! The sidebar: Places (home and the user folders), Devices (mounted drives
//! and shares) and the computer row with free space (DESIGN.md §3.3, P1-T06).
//! Order and hidden entries are kept in app.db.

use std::cell::{OnceCell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use gettextrs::gettext;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib};

use cairn_core::places;

use crate::app_data;
use crate::format;

#[derive(Debug, Clone)]
pub struct SidebarEntry {
    pub key: String,
    pub label: String,
    pub section: &'static str,
    pub file: gio::File,
    pub icon: String,
    pub mount: Option<gio::Mount>,
}

type OpenCallback = Rc<dyn Fn(&gio::File)>;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Sidebar {
        pub places: OnceCell<gtk::ListBox>,
        pub devices_label: OnceCell<gtk::Label>,
        pub devices: OnceCell<gtk::ListBox>,
        pub computer: OnceCell<gtk::Button>,
        pub free_label: OnceCell<gtk::Label>,
        pub level: OnceCell<gtk::LevelBar>,
        pub entries: RefCell<Vec<SidebarEntry>>,
        pub hidden: RefCell<Vec<String>>,
        pub monitor: OnceCell<gio::VolumeMonitor>,
        pub on_open: RefCell<Option<OpenCallback>>,
        pub location: RefCell<Option<gio::File>>,
        /// The icon rail shown while the sidebar is collapsed (ADR 0008).
        pub rail: OnceCell<gtk::Box>,
        pub rail_items: OnceCell<gtk::Box>,
        pub rail_buttons: RefCell<Vec<(String, gtk::Button)>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Sidebar {
        const NAME: &'static str = "CairnSidebar";
        type Type = super::Sidebar;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for Sidebar {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }
    }

    impl WidgetImpl for Sidebar {}
    impl BoxImpl for Sidebar {}
}

glib::wrapper! {
    pub struct Sidebar(ObjectSubclass<imp::Sidebar>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl Default for Sidebar {
    fn default() -> Self {
        glib::Object::new()
    }
}

fn section_label(text: &str) -> gtk::Label {
    let label = gtk::Label::builder().label(text).xalign(0.0).build();
    label.add_css_class("sidebar-heading");
    label
}

impl Sidebar {
    fn build(&self) {
        let imp = self.imp();
        self.set_orientation(gtk::Orientation::Vertical);
        self.set_vexpand(true);
        self.add_css_class("cairn-sidebar");

        // App name row (ADR 0008).
        let badge = gtk::Image::builder().icon_name("folder-symbolic").pixel_size(16).build();
        badge.add_css_class("app-badge");
        let app_name = gtk::Label::builder().label(gettext("Cairn")).xalign(0.0).build();
        app_name.add_css_class("app-name");
        let app_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        app_row.add_css_class("app-row");
        app_row.append(&badge);
        app_row.append(&app_name);
        app_name.set_hexpand(true);
        let collapse = gtk::Button::builder()
            .icon_name("sidebar-show-symbolic")
            .tooltip_text(gettext("Collapse Sidebar"))
            .action_name("win.toggle-sidebar")
            .valign(gtk::Align::Center)
            .build();
        collapse.add_css_class("flat");
        collapse.add_css_class("sidebar-toggle");
        collapse.update_property(&[gtk::accessible::Property::Label(&gettext("Collapse sidebar"))]);
        app_row.append(&collapse);
        let handle = gtk::WindowHandle::builder().child(&app_row).build();
        self.append(&handle);

        let scroller_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let places = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::Single).build();
        places.add_css_class("navigation-sidebar");
        places.update_property(&[gtk::accessible::Property::Label(&gettext("Sidebar"))]);
        let devices_label = section_label(&gettext("Locations"));
        let devices = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::Single).build();
        devices.add_css_class("navigation-sidebar");
        devices.update_property(&[gtk::accessible::Property::Label(&gettext("Locations"))]);
        scroller_box.append(&section_label(&gettext("Favorites")));
        scroller_box.append(&places);
        scroller_box.append(&devices_label);
        scroller_box.append(&devices);
        let scroller = gtk::ScrolledWindow::builder()
            .child(&scroller_box)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build();
        self.append(&scroller);

        // The computer row: free space on the home drive.
        let free_label = gtk::Label::builder().xalign(0.0).build();
        free_label.add_css_class("computer-free");
        let level = gtk::LevelBar::builder().min_value(0.0).max_value(1.0).hexpand(true).build();
        level.add_css_class("thin");
        for offset in [gtk::LEVEL_BAR_OFFSET_LOW, gtk::LEVEL_BAR_OFFSET_HIGH, gtk::LEVEL_BAR_OFFSET_FULL] {
            level.remove_offset_value(Some(offset));
        }
        let title = gtk::Label::builder().label(gettext("Computer")).xalign(0.0).build();
        title.add_css_class("computer-name");
        let usage = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        usage.append(&level);
        usage.append(&free_label);
        let text = gtk::Box::new(gtk::Orientation::Vertical, 4);
        text.append(&title);
        text.append(&usage);
        text.set_hexpand(true);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let computer_icon = gtk::Image::builder().icon_name("computer-symbolic").pixel_size(32).build();
        row.append(&computer_icon);
        row.append(&text);
        let computer = gtk::Button::builder().child(&row).build();
        computer.add_css_class("flat");
        computer.add_css_class("computer-row");
        computer.set_action_name(Some("win.go-to"));
        computer.set_action_target_value(Some(&"/".to_variant()));
        computer.update_property(&[gtk::accessible::Property::Label(&gettext("Computer"))]);
        self.append(&computer);

        for list in [&places, &devices] {
            let weak = self.downgrade();
            list.connect_row_activated(move |_, row| {
                if let Some(sidebar) = weak.upgrade() {
                    sidebar.open_row(row);
                }
            });
            self.add_row_menu(list);
        }

        let monitor = gio::VolumeMonitor::get();
        for signal in ["mount-added", "mount-removed", "mount-changed"] {
            let weak = self.downgrade();
            monitor.connect_local(signal, false, move |_| {
                if let Some(sidebar) = weak.upgrade() {
                    sidebar.rebuild();
                }
                None
            });
        }

        let _ = imp.places.set(places);
        let _ = imp.devices_label.set(devices_label);
        let _ = imp.devices.set(devices);
        let _ = imp.computer.set(computer);
        let _ = imp.free_label.set(free_label);
        let _ = imp.level.set(level);
        let _ = imp.monitor.set(monitor);
        self.build_rail();
        self.rebuild();
        self.refresh_free_space();
    }

    fn build_rail(&self) {
        let imp = self.imp();
        let rail = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .accessible_role(gtk::AccessibleRole::Group)
            .build();
        rail.add_css_class("sidebar-rail");
        rail.update_property(&[gtk::accessible::Property::Label(&gettext("Sidebar rail"))]);
        let badge = gtk::Image::builder().icon_name("folder-symbolic").pixel_size(16).build();
        badge.add_css_class("app-badge");
        let expand = gtk::Button::builder()
            .child(&badge)
            .tooltip_text(gettext("Expand Sidebar"))
            .action_name("win.toggle-sidebar")
            .build();
        expand.add_css_class("flat");
        expand.update_property(&[gtk::accessible::Property::Label(&gettext("Expand sidebar"))]);
        let items = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(4).vexpand(true).build();
        items.set_margin_top(14);
        let computer = gtk::Button::builder()
            .icon_name("computer-symbolic")
            .tooltip_text(gettext("Computer"))
            .action_name("win.go-to")
            .action_target(&"/".to_variant())
            .build();
        computer.add_css_class("flat");
        computer.update_property(&[gtk::accessible::Property::Label(&gettext("Computer"))]);
        rail.append(&gtk::WindowHandle::builder().child(&expand).build());
        rail.append(&items);
        rail.append(&computer);
        let _ = imp.rail.set(rail);
        let _ = imp.rail_items.set(items);
    }

    /// The icon rail, for the window to place while the sidebar is collapsed.
    pub fn rail(&self) -> gtk::Box {
        self.imp().rail.get().expect("rail").clone()
    }

    fn rebuild_rail(&self, entries: &[SidebarEntry]) {
        let imp = self.imp();
        let items = imp.rail_items.get().expect("rail items");
        while let Some(child) = items.first_child() {
            items.remove(&child);
        }
        let mut buttons = Vec::new();
        for entry in entries {
            let target = entry.file.path().map(|p| p.display().to_string()).unwrap_or_else(|| entry.file.uri().to_string());
            let button = gtk::Button::builder()
                .icon_name(&entry.icon)
                .tooltip_text(&entry.label)
                .action_name("win.go-to")
                .action_target(&target.to_variant())
                .build();
            button.add_css_class("flat");
            button.update_property(&[gtk::accessible::Property::Label(&entry.label)]);
            items.append(&button);
            buttons.push((entry.key.clone(), button));
        }
        *imp.rail_buttons.borrow_mut() = buttons;
    }

    pub fn rail_labels(&self) -> Vec<String> {
        self.imp()
            .rail_buttons
            .borrow()
            .iter()
            .filter_map(|(_, b)| b.tooltip_text().map(|t| t.to_string()))
            .collect()
    }

    pub fn connect_open(&self, callback: impl Fn(&gio::File) + 'static) {
        *self.imp().on_open.borrow_mut() = Some(Rc::new(callback));
    }

    fn open_row(&self, row: &gtk::ListBoxRow) {
        let key = row.widget_name().to_string();
        let file = self.imp().entries.borrow().iter().find(|e| e.key == key).map(|e| e.file.clone());
        let callback = self.imp().on_open.borrow().clone();
        if let (Some(file), Some(callback)) = (file, callback) {
            callback(&file);
        }
    }

    /// The default entries, before stored order and hiding are applied.
    fn default_entries(&self) -> Vec<SidebarEntry> {
        let mut entries: Vec<SidebarEntry> = places::standard_places()
            .into_iter()
            .map(|place| SidebarEntry {
                label: if place.key == "place:home" { gettext("Home") } else { place.label },
                key: place.key,
                section: "places",
                file: gio::File::for_path(&place.path),
                icon: place.icon.into(),
                mount: None,
            })
            .collect();
        let monitor = gio::VolumeMonitor::get();
        for mount in monitor.mounts() {
            if mount.is_shadowed() || !(mount.can_unmount() || mount.can_eject()) {
                continue;
            }
            let root = mount.root();
            entries.push(SidebarEntry {
                key: format!("device:{}", root.uri()),
                label: mount.name().to_string(),
                section: "devices",
                file: root,
                icon: mount
                    .symbolic_icon()
                    .downcast::<gio::ThemedIcon>()
                    .ok()
                    .and_then(|icon| icon.names().first().map(|n| n.to_string()))
                    .unwrap_or_else(|| "drive-harddisk-symbolic".into()),
                mount: Some(mount),
            });
        }
        entries
    }

    /// Re-reads places, mounts and stored choices, and rebuilds the rows.
    pub fn rebuild(&self) {
        let imp = self.imp();
        let mut entries = self.default_entries();
        let choices = app_data::store().borrow().sidebar_choices().unwrap_or_default();
        let hidden: Vec<String> = choices.iter().filter(|c| c.hidden).map(|c| c.key.clone()).collect();
        let position = |key: &str| choices.iter().find(|c| c.key == key).and_then(|c| c.position);
        // Stored positions first (in stored order); the rest keep their default order.
        let mut indexed: Vec<(usize, SidebarEntry)> = entries.drain(..).enumerate().collect();
        indexed.sort_by_key(|(index, entry)| match position(&entry.key) {
            Some(p) => (0, p, *index as i64),
            None => (1, 0, *index as i64),
        });
        let entries: Vec<SidebarEntry> = indexed
            .into_iter()
            .map(|(_, e)| e)
            .filter(|e| !hidden.contains(&e.key))
            .collect();

        let (places, devices) = (imp.places.get().unwrap(), imp.devices.get().unwrap());
        places.remove_all();
        devices.remove_all();
        for entry in &entries {
            let row = self.make_row(entry);
            if entry.section == "devices" { devices.append(&row) } else { places.append(&row) }
        }
        let has_devices = entries.iter().any(|e| e.section == "devices");
        imp.devices_label.get().unwrap().set_visible(has_devices);
        devices.set_visible(has_devices);
        self.rebuild_rail(&entries);
        *imp.entries.borrow_mut() = entries;
        *imp.hidden.borrow_mut() = hidden;
        let location = imp.location.borrow().clone();
        if let Some(location) = location {
            self.select_location(&location);
        }
    }

    fn make_row(&self, entry: &SidebarEntry) -> gtk::ListBoxRow {
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        content.append(&gtk::Image::from_icon_name(&entry.icon));
        let label = gtk::Label::builder()
            .label(&entry.label)
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build();
        content.append(&label);
        if let Some(mount) = &entry.mount {
            let eject = gtk::Button::from_icon_name("media-eject-symbolic");
            eject.add_css_class("flat");
            eject.set_tooltip_text(Some(&gettext("Eject")));
            eject.update_property(&[gtk::accessible::Property::Label(&format!(
                "{} {}",
                gettext("Eject"),
                entry.label
            ))]);
            let mount = mount.clone();
            eject.connect_clicked(move |_| {
                let mount = mount.clone();
                glib::spawn_future_local(async move {
                    let result = if mount.can_eject() {
                        mount.eject_with_operation_future(gio::MountUnmountFlags::NONE, gio::MountOperation::NONE).await
                    } else {
                        mount.unmount_with_operation_future(gio::MountUnmountFlags::NONE, gio::MountOperation::NONE).await
                    };
                    if let Err(error) = result {
                        glib::g_warning!("cairn", "could not eject: {error}");
                    }
                });
            });
            content.append(&eject);
        }
        let button = gtk::Button::builder().child(&content).build();
        button.add_css_class("flat");
        button.add_css_class("sidebar-button");
        button.update_property(&[gtk::accessible::Property::Label(&entry.label)]);
        let weak = self.downgrade();
        let file = entry.file.clone();
        button.connect_clicked(move |_| {
            let callback = weak.upgrade().and_then(|s| s.imp().on_open.borrow().clone());
            if let Some(callback) = callback {
                callback(&file);
            }
        });
        let row = gtk::ListBoxRow::builder().child(&button).activatable(false).build();
        row.set_widget_name(&entry.key);
        row.set_tooltip_text(entry.file.path().as_ref().map(|p| p.display().to_string()).as_deref());
        row.update_property(&[gtk::accessible::Property::Label(&entry.label)]);
        row
    }

    /// Highlights the entry for `location`, if there is one.
    pub fn select_location(&self, location: &gio::File) {
        let imp = self.imp();
        *imp.location.borrow_mut() = Some(location.clone());
        let key = imp.entries.borrow().iter().find(|e| e.file.equal(location)).map(|e| e.key.clone());
        for list in [imp.places.get().unwrap(), imp.devices.get().unwrap()] {
            let mut found = None;
            let mut index = 0;
            while let Some(row) = list.row_at_index(index) {
                if Some(row.widget_name().as_str()) == key.as_deref() {
                    found = Some(row);
                }
                index += 1;
            }
            match found {
                Some(row) => list.select_row(Some(&row)),
                None => list.unselect_all(),
            }
        }
        for (entry_key, button) in imp.rail_buttons.borrow().iter() {
            if Some(entry_key.as_str()) == key.as_deref() {
                button.add_css_class("rail-selected");
            } else {
                button.remove_css_class("rail-selected");
            }
        }
    }

    /// Updates the computer row from the home drive's free space.
    pub fn refresh_free_space(&self) {
        let weak = self.downgrade();
        let home = gio::File::for_path(glib::home_dir());
        glib::spawn_future_local(async move {
            let Ok(info) = home
                .query_filesystem_info_future("filesystem::free,filesystem::size", glib::Priority::DEFAULT)
                .await
            else {
                return;
            };
            let Some(sidebar) = weak.upgrade() else {
                return;
            };
            let free = info.attribute_uint64("filesystem::free");
            let size = info.attribute_uint64("filesystem::size");
            let imp = sidebar.imp();
            imp.free_label.get().unwrap().set_label(&format!("{} {}", format::size(free), gettext("free")));
            if size > 0 {
                imp.level.get().unwrap().set_value(1.0 - free as f64 / size as f64);
            }
        });
    }

    // --- changes the user makes -------------------------------------------

    pub fn hide(&self, key: &str) {
        let _ = app_data::store().borrow().set_sidebar_hidden(key, true);
        self.rebuild();
    }

    pub fn move_by(&self, key: &str, delta: i32) {
        let mut keys: Vec<String> = self.imp().entries.borrow().iter().map(|e| e.key.clone()).collect();
        let Some(index) = keys.iter().position(|k| k == key) else {
            return;
        };
        let target = (index as i32 + delta).clamp(0, keys.len() as i32 - 1) as usize;
        let item = keys.remove(index);
        keys.insert(target, item);
        let _ = app_data::store().borrow_mut().set_sidebar_order(&keys);
        self.rebuild();
    }

    pub fn reset(&self) {
        let _ = app_data::store().borrow().reset_sidebar();
        self.rebuild();
    }

    pub fn entries(&self) -> Vec<SidebarEntry> {
        self.imp().entries.borrow().clone()
    }

    pub fn hidden(&self) -> Vec<String> {
        self.imp().hidden.borrow().clone()
    }

    pub fn selected_key(&self) -> Option<String> {
        let imp = self.imp();
        [imp.places.get().unwrap(), imp.devices.get().unwrap()]
            .into_iter()
            .find_map(|list| list.selected_row())
            .map(|row| row.widget_name().to_string())
    }

    fn add_row_menu(&self, list: &gtk::ListBox) {
        let click = gtk::GestureClick::builder().button(gdk::BUTTON_SECONDARY).build();
        let list_weak = list.downgrade();
        click.connect_pressed(move |_, _, x, y| {
            let Some(list) = list_weak.upgrade() else {
                return;
            };
            let Some(row) = list.row_at_y(y as i32) else {
                return;
            };
            let key = row.widget_name().to_string();
            let menu = gio::Menu::new();
            let item = |label: String, action: &str| {
                let item = gio::MenuItem::new(Some(&label), None);
                item.set_action_and_target_value(Some(action), Some(&key.to_variant()));
                item
            };
            menu.append_item(&item(gettext("Move _Up"), "win.sidebar-move-up"));
            menu.append_item(&item(gettext("Move _Down"), "win.sidebar-move-down"));
            menu.append_item(&item(gettext("_Hide"), "win.sidebar-hide"));
            let popover = gtk::PopoverMenu::from_model(Some(&menu));
            popover.set_parent(&list);
            popover.set_has_arrow(false);
            popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            popover.connect_closed(|popover| {
                let popover = popover.clone();
                glib::idle_add_local_once(move || popover.unparent());
            });
            popover.popup();
        });
        list.add_controller(click);
    }
}

/// Paths of the entries, for tests and diagnostics.
pub fn entry_path(entry: &SidebarEntry) -> Option<PathBuf> {
    entry.file.path()
}
