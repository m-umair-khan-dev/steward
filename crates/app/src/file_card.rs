//! A file shown as a card (ADR 0008): a preview area on top (the real
//! thumbnail, or the file's icon), then a coloured type badge, the name and
//! "Kind · size".

use std::cell::{OnceCell, RefCell};

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib};

use cairn_core::FileEntry;

use crate::folder_view::RowSize;
use crate::{format, tint};

/// Width of a file card for each size.
pub fn card_width(size: RowSize) -> i32 {
    match size {
        RowSize::Small => 170,
        RowSize::Medium => 214,
        RowSize::Large => 272,
    }
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct FileCard {
        pub preview: OnceCell<gtk::Overlay>,
        pub image: OnceCell<gtk::Image>,
        pub picture: OnceCell<gtk::Picture>,
        pub badge: OnceCell<gtk::Label>,
        pub name: OnceCell<gtk::Label>,
        pub meta: OnceCell<gtk::Label>,
        pub entry: RefCell<Option<FileEntry>>,
        pub handler: RefCell<Option<glib::SignalHandlerId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FileCard {
        const NAME: &'static str = "CairnFileCard";
        type Type = super::FileCard;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for FileCard {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }
    }

    impl WidgetImpl for FileCard {}
    impl BoxImpl for FileCard {}
}

glib::wrapper! {
    pub struct FileCard(ObjectSubclass<imp::FileCard>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl Default for FileCard {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl FileCard {
    fn build(&self) {
        let imp = self.imp();
        self.set_orientation(gtk::Orientation::Vertical);
        self.set_hexpand(false);
        self.set_halign(gtk::Align::Start);
        self.add_css_class("file-card");
        self.set_overflow(gtk::Overflow::Hidden);

        let image = gtk::Image::builder().halign(gtk::Align::Center).valign(gtk::Align::Center).build();
        let picture = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Cover)
            .can_shrink(true)
            .visible(false)
            .build();
        let preview = gtk::Overlay::builder().child(&image).build();
        preview.add_overlay(&picture);
        preview.add_css_class("file-preview");

        let badge = gtk::Label::builder().valign(gtk::Align::Center).build();
        badge.add_css_class("file-badge");
        // max-width-chars 1 keeps the labels from widening the card; they
        // fill the card's fixed width and ellipsize inside it.
        let name = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .max_width_chars(1)
            .hexpand(true)
            .build();
        name.add_css_class("file-name");
        let meta = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(1)
            .hexpand(true)
            .build();
        meta.add_css_class("file-meta");
        let text = gtk::Box::builder().orientation(gtk::Orientation::Vertical).hexpand(true).build();
        text.append(&name);
        text.append(&meta);
        let info = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        info.add_css_class("file-info");
        info.append(&badge);
        info.append(&text);

        self.append(&preview);
        self.append(&info);
        let _ = imp.preview.set(preview);
        let _ = imp.image.set(image);
        let _ = imp.picture.set(picture);
        let _ = imp.badge.set(badge);
        let _ = imp.name.set(name);
        let _ = imp.meta.set(meta);
    }

    pub fn bind(&self, entry: &FileEntry, size: RowSize) {
        self.unbind();
        let imp = self.imp();
        let width = card_width(size);
        self.set_size_request(width, -1);
        imp.preview.get().unwrap().set_size_request(width, width * 9 / 16);
        let data = entry.data();
        for class in tint::ALL_CLASSES {
            self.remove_css_class(class);
        }
        self.add_css_class(tint::class(data.kind));
        imp.badge.get().unwrap().set_label(&tint::badge(&data.name, data.kind));
        imp.name.get().unwrap().set_label(&data.display_name);
        let description = gtk::gio::content_type_get_description(&data.content_type);
        imp.meta.get().unwrap().set_label(&format!("{description} · {}", format::size(data.size)));
        let image = imp.image.get().unwrap();
        image.set_pixel_size((width / 4).max(32));
        match &data.icon {
            Some(icon) => image.set_from_gicon(icon),
            None => image.set_icon_name(Some("text-x-generic")),
        }
        self.set_tooltip_text(Some(&data.display_name));
        self.update_property(&[gtk::accessible::Property::Label(&data.display_name)]);

        let weak = self.downgrade();
        let handler = entry.connect_notify_local(Some("thumbnail-path"), move |_, _| {
            if let Some(card) = weak.upgrade() {
                card.show_thumbnail();
            }
        });
        *imp.handler.borrow_mut() = Some(handler);
        *imp.entry.borrow_mut() = Some(entry.clone());
        self.show_thumbnail();
    }

    pub fn unbind(&self) {
        let imp = self.imp();
        if let (Some(entry), Some(handler)) = (imp.entry.borrow_mut().take(), imp.handler.borrow_mut().take()) {
            entry.disconnect(handler);
        }
    }

    pub fn entry(&self) -> Option<FileEntry> {
        self.imp().entry.borrow().clone()
    }

    /// Whether the card shows a real preview rather than an icon.
    pub fn has_thumbnail(&self) -> bool {
        self.imp().picture.get().is_some_and(|p| p.is_visible() && p.paintable().is_some())
    }

    pub fn set_selected_look(&self, selected: bool) {
        if selected {
            self.add_css_class("selected-card");
        } else {
            self.remove_css_class("selected-card");
        }
    }

    fn show_thumbnail(&self) {
        let imp = self.imp();
        let (image, picture) = (imp.image.get().unwrap(), imp.picture.get().unwrap());
        let path = imp.entry.borrow().as_ref().and_then(FileEntry::thumbnail_path);
        if let Some(texture) = path.and_then(|p| gdk::Texture::from_filename(p).ok()) {
            picture.set_paintable(Some(&texture));
            picture.set_visible(true);
            image.set_visible(false);
        } else {
            picture.set_paintable(None::<&gdk::Paintable>);
            picture.set_visible(false);
            image.set_visible(true);
        }
    }
}
