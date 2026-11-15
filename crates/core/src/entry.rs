//! One item in a folder listing, as a GObject so list widgets can show it.

use std::cell::{OnceCell, RefCell};

use gio::prelude::*;
use glib::subclass::prelude::*;

use crate::kind::Kind;

/// File attributes every listing asks GIO for.
pub const ATTRIBUTES: &str = "standard::name,standard::display-name,standard::type,\
standard::size,standard::is-hidden,standard::is-backup,standard::is-symlink,\
standard::fast-content-type,standard::content-type,standard::icon,standard::symbolic-icon,\
time::modified,time::created,access::can-read,thumbnail::path,thumbnail::is-valid";

/// Plain data about a file, taken from a `gio::FileInfo`.
#[derive(Debug, Clone)]
pub struct EntryData {
    pub file: gio::File,
    pub name: String,
    pub display_name: String,
    pub is_dir: bool,
    pub is_hidden: bool,
    pub is_symlink: bool,
    pub size: u64,
    /// Seconds since the epoch; 0 when unknown.
    pub modified: i64,
    /// Birth time in seconds since the epoch; 0 when the filesystem does not record it.
    pub created: i64,
    pub content_type: String,
    pub kind: Kind,
    pub icon: Option<gio::Icon>,
    pub symbolic_icon: Option<gio::Icon>,
    pub can_read: bool,
}

impl EntryData {
    pub fn from_info(parent: &gio::File, info: &gio::FileInfo) -> Self {
        let name = info.name().to_string_lossy().into_owned();
        let file = parent.child(&name);
        let is_dir = info.file_type() == gio::FileType::Directory;
        let content_type = if is_dir {
            "inode/directory".to_string()
        } else {
            info.attribute_string("standard::fast-content-type")
                .or_else(|| info.content_type())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "application/octet-stream".into())
        };
        let kind = Kind::from_content_type(&content_type);
        let display_name = info.display_name().to_string();
        let is_hidden = info.is_hidden() || info.is_backup() || name.starts_with('.');
        let has = |attr: &str| info.has_attribute(attr);
        Self {
            file,
            name,
            display_name,
            is_dir,
            is_hidden,
            is_symlink: info.is_symlink(),
            size: if is_dir { 0 } else { info.size().max(0) as u64 },
            modified: if has("time::modified") {
                info.attribute_uint64("time::modified") as i64
            } else {
                0
            },
            created: if has("time::created") {
                info.attribute_uint64("time::created") as i64
            } else {
                0
            },
            content_type,
            kind,
            icon: info.icon(),
            symbolic_icon: info.symbolic_icon(),
            can_read: !has("access::can-read") || info.boolean("access::can-read"),
        }
    }

    pub fn uri(&self) -> String {
        self.file.uri().to_string()
    }
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct FileEntry {
        pub data: OnceCell<EntryData>,
        pub thumbnail_path: RefCell<Option<String>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FileEntry {
        const NAME: &'static str = "CairnFileEntry";
        type Type = super::FileEntry;
    }

    impl ObjectImpl for FileEntry {
        fn properties() -> &'static [glib::ParamSpec] {
            use std::sync::OnceLock;
            static PROPERTIES: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPERTIES.get_or_init(|| {
                vec![glib::ParamSpecString::builder("thumbnail-path").build()]
            })
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            if pspec.name() == "thumbnail-path" {
                *self.thumbnail_path.borrow_mut() = value.get().expect("string");
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "thumbnail-path" => self.thumbnail_path.borrow().to_value(),
                _ => unreachable!(),
            }
        }
    }
}

glib::wrapper! {
    /// A file or folder in a listing. Its data never changes; when the file
    /// changes on disk, the listing replaces the entry with a new one.
    pub struct FileEntry(ObjectSubclass<imp::FileEntry>);
}

impl FileEntry {
    pub fn new(data: EntryData) -> Self {
        let entry: Self = glib::Object::new();
        let thumbnail = None::<String>;
        entry.imp().data.set(data).expect("set once");
        *entry.imp().thumbnail_path.borrow_mut() = thumbnail;
        entry
    }

    pub fn from_info(parent: &gio::File, info: &gio::FileInfo) -> Self {
        let entry = Self::new(EntryData::from_info(parent, info));
        let valid = !info.has_attribute("thumbnail::is-valid") || info.boolean("thumbnail::is-valid");
        if valid {
            if let Some(path) = info.attribute_byte_string("thumbnail::path") {
                *entry.imp().thumbnail_path.borrow_mut() = Some(path.to_string());
            }
        }
        entry
    }

    pub fn data(&self) -> &EntryData {
        self.imp().data.get().expect("entry has data")
    }

    pub fn name(&self) -> &str {
        &self.data().name
    }

    pub fn display_name(&self) -> &str {
        &self.data().display_name
    }

    pub fn kind(&self) -> Kind {
        self.data().kind
    }

    pub fn is_dir(&self) -> bool {
        self.data().is_dir
    }

    pub fn thumbnail_path(&self) -> Option<String> {
        self.imp().thumbnail_path.borrow().clone()
    }

    /// Records a thumbnail and notifies widgets showing this entry.
    pub fn set_thumbnail_path(&self, path: Option<&str>) {
        self.set_property("thumbnail-path", path);
    }
}
