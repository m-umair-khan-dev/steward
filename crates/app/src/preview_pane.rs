//! The preview pane: facts about the selected file, or for a folder, what is
//! inside it (DESIGN.md §3.3, P1-T09). Folder trees two levels deep arrive
//! with the index in Phase 3; Phase 1 shows the direct children.

use std::cell::{Cell, OnceCell, RefCell};

use gettextrs::gettext;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib};

use cairn_core::{FileEntry, Kind, natural};

use crate::format;
use crate::path_bar;
use crate::tint;

const MAX_CHILDREN: usize = 12;
const MAX_KINDS: usize = 5;

/// What the pane shows about the open folder when nothing is selected.
#[derive(Debug, Clone, PartialEq)]
pub struct Overview {
    pub title: String,
    pub bytes: u64,
    pub folders: u32,
    pub files: u32,
    /// Files by kind: kind, count, bytes.
    pub kinds: Vec<(Kind, u32, u64)>,
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct PreviewPane {
        pub image: OnceCell<gtk::Image>,
        pub picture: OnceCell<gtk::Picture>,
        pub name: OnceCell<gtk::Label>,
        pub facts: OnceCell<gtk::Grid>,
        pub children_title: OnceCell<gtk::Label>,
        pub children: OnceCell<gtk::Box>,
        pub open: OnceCell<gtk::Button>,
        pub placeholder: OnceCell<gtk::Label>,
        pub overview_box: OnceCell<gtk::Box>,
        pub overview: RefCell<Option<Overview>>,
        pub shown: RefCell<Option<FileEntry>>,
        pub fact_rows: RefCell<Vec<(String, String)>>,
        pub child_names: RefCell<Vec<String>>,
        pub generation: Cell<u64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PreviewPane {
        const NAME: &'static str = "CairnPreviewPane";
        type Type = super::PreviewPane;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for PreviewPane {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }
    }

    impl WidgetImpl for PreviewPane {}
    impl BoxImpl for PreviewPane {}
}

glib::wrapper! {
    pub struct PreviewPane(ObjectSubclass<imp::PreviewPane>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl Default for PreviewPane {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl PreviewPane {
    fn build(&self) {
        let imp = self.imp();
        self.set_orientation(gtk::Orientation::Vertical);
        self.set_spacing(12);
        self.set_width_request(260);
        self.add_css_class("preview-pane");
        self.update_property(&[gtk::accessible::Property::Label(&gettext("Preview"))]);

        let placeholder = gtk::Label::builder()
            .label(gettext("Select a file to see it here"))
            .wrap(true)
            .vexpand(true)
            .build();
        placeholder.add_css_class("dim-label");
        let image = gtk::Image::builder().pixel_size(96).visible(false).build();
        let picture = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Contain)
            .height_request(160)
            .visible(false)
            .build();
        let name = gtk::Label::builder().wrap(true).wrap_mode(gtk::pango::WrapMode::WordChar).visible(false).build();
        name.add_css_class("title-4");
        let facts = gtk::Grid::builder().column_spacing(12).row_spacing(4).visible(false).build();
        let children_title = gtk::Label::builder().xalign(0.0).visible(false).build();
        children_title.add_css_class("heading");
        let children = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let open = gtk::Button::builder().label(gettext("Open")).halign(gtk::Align::Start).visible(false).build();
        open.set_action_name(Some("win.open-selected"));

        let overview_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .visible(false)
            .build();
        overview_box.update_property(&[gtk::accessible::Property::Label(&gettext("Folder overview"))]);

        for widget in [
            overview_box.upcast_ref::<gtk::Widget>(),
            placeholder.upcast_ref::<gtk::Widget>(),
            image.upcast_ref(),
            picture.upcast_ref(),
            name.upcast_ref(),
            facts.upcast_ref(),
            children_title.upcast_ref(),
            children.upcast_ref(),
            open.upcast_ref(),
        ] {
            self.append(widget);
        }
        let _ = imp.image.set(image);
        let _ = imp.picture.set(picture);
        let _ = imp.name.set(name);
        let _ = imp.facts.set(facts);
        let _ = imp.children_title.set(children_title);
        let _ = imp.children.set(children);
        let _ = imp.open.set(open);
        let _ = imp.placeholder.set(placeholder);
        let _ = imp.overview_box.set(overview_box);
    }

    /// Sets the folder overview shown when nothing is selected.
    pub fn set_overview(&self, overview: Option<Overview>) {
        let imp = self.imp();
        if *imp.overview.borrow() == overview {
            return;
        }
        *imp.overview.borrow_mut() = overview.clone();
        let container = imp.overview_box.get().unwrap();
        while let Some(child) = container.first_child() {
            container.remove(&child);
        }
        if let Some(o) = overview {
            let title = gtk::Label::builder().label(&o.title).xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).build();
            title.add_css_class("title-4");
            let total = gtk::Label::builder().label(format::size(o.bytes)).xalign(0.0).build();
            total.add_css_class("overview-total");
            let in_all = gtk::Label::builder().label(gettext("in all")).valign(gtk::Align::BaselineFill).build();
            in_all.add_css_class("overview-in-all");
            let total_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            total_row.append(&total);
            total_row.append(&in_all);
            let counts = gtk::Label::builder()
                .label(format!("{} · {}", format::folders(o.folders), format::files(o.files)))
                .xalign(0.0)
                .build();
            counts.add_css_class("dim-label");
            container.append(&title);
            container.append(&total_row);
            container.append(&counts);
            if !o.kinds.is_empty() {
                let heading = gtk::Label::builder().label(gettext("WHAT TAKES THE SPACE")).xalign(0.0).build();
                heading.add_css_class("overview-heading");
                container.append(&heading);
                let mut kinds = o.kinds.clone();
                kinds.sort_by(|a, b| b.2.cmp(&a.2));
                let largest = kinds.first().map(|k| k.2).unwrap_or(1).max(1);
                for (kind, _, bytes) in kinds.iter().take(MAX_KINDS) {
                    container.append(&space_row(*kind, *bytes, largest));
                }
                if kinds.len() > MAX_KINDS {
                    let more = gtk::Label::builder()
                        .label(format!("{} {}", gettext("and"), kinds.len() - MAX_KINDS) + " " + &gettext("more"))
                        .xalign(0.0)
                        .build();
                    more.add_css_class("dim-label");
                    container.append(&more);
                }
            }
        }
        self.sync_placeholder();
    }

    fn sync_placeholder(&self) {
        let imp = self.imp();
        let nothing_selected = imp.shown.borrow().is_none();
        let has_overview = imp.overview.borrow().is_some();
        imp.overview_box.get().unwrap().set_visible(nothing_selected && has_overview);
        imp.placeholder.get().unwrap().set_visible(nothing_selected && !has_overview);
    }

    pub fn overview(&self) -> Option<Overview> {
        self.imp().overview.borrow().clone()
    }

    /// Shows `entry`, or the placeholder when nothing (or several) is selected.
    pub fn show(&self, entry: Option<&FileEntry>) {
        let imp = self.imp();
        if imp.shown.borrow().as_ref() == entry {
            return;
        }
        *imp.shown.borrow_mut() = entry.cloned();
        imp.generation.set(imp.generation.get() + 1);
        let children = imp.children.get().unwrap();
        while let Some(child) = children.first_child() {
            children.remove(&child);
        }
        imp.child_names.borrow_mut().clear();
        let has = entry.is_some();
        self.sync_placeholder();
        for widget in [imp.name.get().unwrap().upcast_ref::<gtk::Widget>(), imp.facts.get().unwrap().upcast_ref(), imp.open.get().unwrap().upcast_ref()] {
            widget.set_visible(has);
        }
        imp.children_title.get().unwrap().set_visible(false);
        let Some(entry) = entry else {
            imp.image.get().unwrap().set_visible(false);
            imp.picture.get().unwrap().set_visible(false);
            imp.fact_rows.borrow_mut().clear();
            return;
        };
        let data = entry.data();
        imp.name.get().unwrap().set_label(&data.display_name);
        self.show_image(entry);

        let kind = if data.is_dir {
            gettext("Folder")
        } else {
            gio::content_type_get_description(&data.content_type).to_string()
        };
        let mut rows = vec![(gettext("Kind"), kind)];
        if !data.is_dir {
            rows.push((gettext("Size"), format::size(data.size)));
        }
        rows.push((gettext("Modified"), format::date(data.modified)));
        rows.push((gettext("Created"), format::date(data.created)));
        if let Some(parent) = data.file.parent().and_then(|p| p.path()) {
            rows.push((gettext("Where"), path_bar::display_path(&parent)));
        }
        let facts = imp.facts.get().unwrap();
        while let Some(child) = facts.first_child() {
            facts.remove(&child);
        }
        for (row, (key, value)) in rows.iter().enumerate() {
            let k = gtk::Label::builder().label(key).xalign(1.0).build();
            k.add_css_class("dim-label");
            let v = gtk::Label::builder().label(value).xalign(0.0).wrap(true).selectable(true).build();
            facts.attach(&k, 0, row as i32, 1, 1);
            facts.attach(&v, 1, row as i32, 1, 1);
        }
        *imp.fact_rows.borrow_mut() = rows;
        if data.is_dir {
            self.list_children(&data.file);
        }
    }

    fn show_image(&self, entry: &FileEntry) {
        let imp = self.imp();
        let (image, picture) = (imp.image.get().unwrap(), imp.picture.get().unwrap());
        if let Some(texture) = entry.thumbnail_path().and_then(|p| gdk::Texture::from_filename(p).ok()) {
            picture.set_paintable(Some(&texture));
            picture.set_visible(true);
            image.set_visible(false);
        } else {
            match &entry.data().icon {
                Some(icon) => image.set_from_gicon(icon),
                None => image.set_icon_name(Some("text-x-generic")),
            }
            image.set_visible(true);
            picture.set_visible(false);
        }
    }

    fn list_children(&self, folder: &gio::File) {
        let generation = self.imp().generation.get();
        let weak = self.downgrade();
        let folder = folder.clone();
        glib::spawn_future_local(async move {
            let mut names = Vec::new();
            if let Ok(enumerator) = folder
                .enumerate_children_future("standard::name,standard::is-hidden", gio::FileQueryInfoFlags::NONE, glib::Priority::DEFAULT)
                .await
            {
                while let Ok(infos) = enumerator.next_files_future(200, glib::Priority::DEFAULT).await {
                    if infos.is_empty() {
                        break;
                    }
                    names.extend(
                        infos
                            .iter()
                            .filter(|i| !i.is_hidden() && !i.name().to_string_lossy().starts_with('.'))
                            .map(|i| i.name().to_string_lossy().into_owned()),
                    );
                }
            }
            let Some(pane) = weak.upgrade() else {
                return;
            };
            if pane.imp().generation.get() != generation {
                return;
            }
            names.sort_by(|a, b| natural::compare(a, b));
            pane.show_children(names);
        });
    }

    fn show_children(&self, names: Vec<String>) {
        let imp = self.imp();
        let title = imp.children_title.get().unwrap();
        title.set_label(&format::items(names.len() as u32));
        title.set_visible(true);
        let children = imp.children.get().unwrap();
        for name in names.iter().take(MAX_CHILDREN) {
            let label = gtk::Label::builder().label(name).xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::Middle).build();
            children.append(&label);
        }
        if names.len() > MAX_CHILDREN {
            let more = gtk::Label::builder()
                .label(format!("{} {}", gettext("and"), format::items((names.len() - MAX_CHILDREN) as u32)))
                .xalign(0.0)
                .build();
            more.add_css_class("dim-label");
            children.append(&more);
        }
        *imp.child_names.borrow_mut() = names;
    }

    pub fn shown(&self) -> Option<FileEntry> {
        self.imp().shown.borrow().clone()
    }

    pub fn facts(&self) -> Vec<(String, String)> {
        self.imp().fact_rows.borrow().clone()
    }

    pub fn children(&self) -> Vec<String> {
        self.imp().child_names.borrow().clone()
    }
}

/// One "what takes the space" row: coloured dot, kind, size, and a bar.
fn space_row(kind: Kind, bytes: u64, largest: u64) -> gtk::Box {
    let dot = gtk::Box::builder().valign(gtk::Align::Center).build();
    dot.add_css_class("space-dot");
    dot.add_css_class(tint::class(kind));
    let name = gtk::Label::builder().label(format::kind(kind)).xalign(0.0).hexpand(true).build();
    let size = gtk::Label::builder().label(format::size(bytes)).build();
    size.add_css_class("dim-label");
    size.add_css_class("numeric");
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    line.append(&dot);
    line.append(&name);
    line.append(&size);
    let bar = gtk::LevelBar::builder()
        .min_value(0.0)
        .max_value(1.0)
        .value(bytes as f64 / largest as f64)
        .build();
    bar.add_css_class("thin");
    bar.add_css_class(tint::class(kind));
    for offset in [gtk::LEVEL_BAR_OFFSET_LOW, gtk::LEVEL_BAR_OFFSET_HIGH, gtk::LEVEL_BAR_OFFSET_FULL] {
        bar.remove_offset_value(Some(offset));
    }
    let row = gtk::Box::new(gtk::Orientation::Vertical, 4);
    row.add_css_class("overview-row");
    row.append(&line);
    row.append(&bar);
    row.update_property(&[gtk::accessible::Property::Label(&format!("{}, {}", format::kind(kind), format::size(bytes)))]);
    row
}
