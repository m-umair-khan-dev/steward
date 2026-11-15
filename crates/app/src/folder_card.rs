//! A folder drawn as a tinted "glass" folder (ADR 0008): coloured back with a
//! tab, two sheets of paper, and a frosted front showing how many items it
//! holds and a symbol. Also used for the virtual kind folders of "Group by
//! Kind" ("3 files", "1.6 GB"). Built from plain widgets and CSS.

use std::cell::{Cell, OnceCell, RefCell};

use gettextrs::ngettext;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

use cairn_core::{FileEntry, Kind};

use crate::folder_view::RowSize;
use crate::{format, tint};

/// Width of the folder shape for each size.
pub fn shape_width(size: RowSize) -> i32 {
    match size {
        RowSize::Small => 104,
        RowSize::Medium => 132,
        RowSize::Large => 164,
    }
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct FolderCard {
        pub shape: OnceCell<gtk::Overlay>,
        pub back: OnceCell<gtk::Box>,
        pub tab: OnceCell<gtk::Box>,
        pub papers: OnceCell<[gtk::Box; 2]>,
        pub front: OnceCell<gtk::Box>,
        pub count: OnceCell<gtk::Label>,
        pub symbol: OnceCell<gtk::Image>,
        pub size: OnceCell<gtk::Label>,
        pub name: OnceCell<gtk::Label>,
        pub entry: RefCell<Option<FileEntry>>,
        pub kind: Cell<Option<Kind>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FolderCard {
        const NAME: &'static str = "CairnFolderCard";
        type Type = super::FolderCard;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for FolderCard {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }
    }

    impl WidgetImpl for FolderCard {}
    impl BoxImpl for FolderCard {}
}

glib::wrapper! {
    pub struct FolderCard(ObjectSubclass<imp::FolderCard>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl Default for FolderCard {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl FolderCard {
    fn build(&self) {
        let imp = self.imp();
        self.set_orientation(gtk::Orientation::Vertical);
        self.set_spacing(6);
        self.set_hexpand(false);
        self.set_halign(gtk::Align::Start);
        self.add_css_class("folder-card");

        let back = gtk::Box::builder().vexpand(true).hexpand(true).margin_top(12).build();
        back.add_css_class("folder-back");
        let tab = gtk::Box::builder().halign(gtk::Align::Start).valign(gtk::Align::Start).build();
        tab.add_css_class("folder-tab");
        let paper = |class: &str| {
            let paper = gtk::Box::builder().halign(gtk::Align::Center).valign(gtk::Align::Start).build();
            paper.add_css_class("folder-paper");
            paper.add_css_class(class);
            paper
        };
        let papers = [paper("first"), paper("second")];

        let count = gtk::Label::builder().xalign(0.0).hexpand(true).build();
        count.add_css_class("folder-count");
        let symbol = gtk::Image::builder().vexpand(true).valign(gtk::Align::Center).build();
        symbol.add_css_class("folder-symbol");
        let size = gtk::Label::builder().xalign(0.0).build();
        size.add_css_class("folder-size");
        let front = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .valign(gtk::Align::End)
            .build();
        front.add_css_class("folder-front");
        front.append(&count);
        front.append(&symbol);
        front.append(&size);

        let shape = gtk::Overlay::builder().child(&back).halign(gtk::Align::Center).build();
        shape.add_overlay(&tab);
        for paper in &papers {
            shape.add_overlay(paper);
        }
        shape.add_overlay(&front);

        let name = gtk::Label::builder()
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .lines(2)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .justify(gtk::Justification::Center)
            .max_width_chars(16)
            .build();
        name.add_css_class("folder-name");
        self.append(&shape);
        self.append(&name);

        let _ = imp.shape.set(shape);
        let _ = imp.back.set(back);
        let _ = imp.tab.set(tab);
        let _ = imp.papers.set(papers);
        let _ = imp.front.set(front);
        let _ = imp.count.set(count);
        let _ = imp.symbol.set(symbol);
        let _ = imp.size.set(size);
        let _ = imp.name.set(name);
    }

    fn apply_size(&self, size: RowSize) {
        let imp = self.imp();
        let width = shape_width(size);
        let height = width * 4 / 5;
        imp.shape.get().unwrap().set_size_request(width, height);
        imp.tab.get().unwrap().set_size_request(width * 2 / 5, 24);
        for (index, paper) in imp.papers.get().unwrap().iter().enumerate() {
            paper.set_size_request(width * 3 / 4, height / 2);
            paper.set_margin_top(16 + index as i32 * 3);
        }
        imp.front.get().unwrap().set_size_request(width, height * 11 / 16);
        imp.symbol.get().unwrap().set_pixel_size((width / 6).max(16));
        self.set_size_request(width + 12, -1);
    }

    fn set_tint(&self, class: &str) {
        for tint in tint::ALL_CLASSES {
            self.remove_css_class(tint);
        }
        self.add_css_class(class);
    }

    /// Shows a real folder. The item count arrives later through `set_count`.
    pub fn bind_entry(&self, entry: &FileEntry, size: RowSize) {
        let imp = self.imp();
        self.apply_size(size);
        self.set_tint(tint::class(Kind::Folder));
        let data = entry.data();
        imp.name.get().unwrap().set_label(&data.display_name);
        let symbol = imp.symbol.get().unwrap();
        match data.symbolic_icon.as_ref().filter(|icon| !is_plain_folder(icon)) {
            Some(icon) => symbol.set_from_gicon(icon),
            None => symbol.set_icon_name(Some("text-x-generic-symbolic")),
        }
        imp.size.get().unwrap().set_label("");
        imp.count.get().unwrap().set_label("");
        self.set_tooltip_text(Some(&data.display_name));
        self.update_property(&[gtk::accessible::Property::Label(&data.display_name)]);
        *imp.entry.borrow_mut() = Some(entry.clone());
        imp.kind.set(None);
    }

    /// Shows a virtual folder holding every file of one kind.
    pub fn bind_kind(&self, kind: Kind, files: u32, bytes: u64, size: RowSize) {
        let imp = self.imp();
        self.apply_size(size);
        self.set_tint(tint::class(kind));
        let label = format::kind(kind);
        imp.name.get().unwrap().set_label(&label);
        imp.symbol.get().unwrap().set_icon_name(Some(tint::symbol(kind)));
        imp.count.get().unwrap().set_label(&format::files(files));
        imp.size.get().unwrap().set_label(&format::size(bytes));
        let a11y = format!("{label}, {}", format::files(files));
        self.set_tooltip_text(Some(&a11y));
        self.update_property(&[gtk::accessible::Property::Label(&label)]);
        *imp.entry.borrow_mut() = None;
        imp.kind.set(Some(kind));
    }

    pub fn set_count(&self, count: Option<u32>) {
        let text = match count {
            Some(n) => ngettext("{} item", "{} items", n).replace("{}", &n.to_string()),
            None => String::new(),
        };
        self.imp().count.get().unwrap().set_label(&text);
    }

    pub fn count_label(&self) -> String {
        self.imp().count.get().map(|l| l.label().to_string()).unwrap_or_default()
    }

    pub fn unbind(&self) {
        *self.imp().entry.borrow_mut() = None;
        self.imp().kind.set(None);
    }

    pub fn entry(&self) -> Option<FileEntry> {
        self.imp().entry.borrow().clone()
    }

    pub fn kind(&self) -> Option<Kind> {
        self.imp().kind.get()
    }

    pub fn name(&self) -> String {
        self.imp().name.get().map(|l| l.label().to_string()).unwrap_or_default()
    }

    pub fn set_selected_look(&self, selected: bool) {
        if selected {
            self.add_css_class("selected-card");
        } else {
            self.remove_css_class("selected-card");
        }
    }
}

fn is_plain_folder(icon: &gio::Icon) -> bool {
    icon.downcast_ref::<gio::ThemedIcon>()
        .and_then(|themed| themed.names().first().map(|n| n.to_string()))
        .is_some_and(|name| {
            matches!(name.as_str(), "folder-symbolic" | "folder" | "inode-directory-symbolic" | "inode-directory")
        })
}
