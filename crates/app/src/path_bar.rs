//! The path bar: breadcrumbs, which turn into a text field on Ctrl+L. The
//! field takes a path (with `~`), and Tab completes folder names (P1-T05).

use std::cell::{OnceCell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib};

type FileCallback = Rc<dyn Fn(&gio::File)>;
type TextCallback = Rc<dyn Fn(&str)>;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct PathBar {
        pub stack: OnceCell<gtk::Stack>,
        pub crumbs: OnceCell<gtk::Box>,
        pub entry: OnceCell<gtk::Entry>,
        pub location: RefCell<Option<gio::File>>,
        pub on_navigate: RefCell<Option<FileCallback>>,
        pub on_invalid: RefCell<Option<TextCallback>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PathBar {
        const NAME: &'static str = "CairnPathBar";
        type Type = super::PathBar;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for PathBar {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }
    }

    impl WidgetImpl for PathBar {}
    impl adw::subclass::prelude::BinImpl for PathBar {}
}

glib::wrapper! {
    pub struct PathBar(ObjectSubclass<imp::PathBar>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for PathBar {
    fn default() -> Self {
        glib::Object::new()
    }
}

/// `~/Documents` for folders under home, the full path otherwise.
pub fn display_path(path: &Path) -> String {
    let home = glib::home_dir();
    match path.strip_prefix(&home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

/// Expands a leading `~` and makes relative paths relative to `base`.
pub fn expand(text: &str, base: Option<&Path>) -> PathBuf {
    let text = text.trim();
    let home = glib::home_dir();
    if text == "~" {
        return home;
    }
    if let Some(rest) = text.strip_prefix("~/") {
        return home.join(rest);
    }
    let path = PathBuf::from(text);
    if path.is_absolute() {
        path
    } else {
        base.map(|b| b.join(&path)).unwrap_or(path)
    }
}

/// Completes the last part of `text` to a folder name inside its parent.
/// One match: the full name plus "/". Several: their longest common prefix.
pub fn complete(text: &str, base: Option<&Path>) -> Option<String> {
    let (dir_part, prefix) = match text.rfind('/') {
        Some(i) => (&text[..=i], &text[i + 1..]),
        None if text == "~" => return Some("~/".into()),
        None => ("", text),
    };
    let dir = if dir_part.is_empty() { base?.to_path_buf() } else { expand(dir_part, base) };
    let mut matches: Vec<String> = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()) || e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(prefix) && (prefix.starts_with('.') || !name.starts_with('.')))
        .collect();
    if matches.is_empty() {
        let lower = prefix.to_lowercase();
        matches = std::fs::read_dir(&dir)
            .ok()?
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name.to_lowercase().starts_with(&lower) && !name.starts_with('.'))
            .collect();
    }
    matches.sort();
    match matches.as_slice() {
        [] => None,
        [one] => Some(format!("{dir_part}{one}/")),
        many => {
            let first = &many[0];
            let common = first
                .char_indices()
                .map(|(i, c)| i + c.len_utf8())
                .take_while(|&end| many.iter().all(|m| m.starts_with(&first[..end])))
                .last()
                .unwrap_or(0);
            (common > prefix.len()).then(|| format!("{dir_part}{}", &first[..common]))
        }
    }
}

impl PathBar {
    fn build(&self) {
        let imp = self.imp();
        let crumbs = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        crumbs.add_css_class("path-crumbs");
        crumbs.update_property(&[gtk::accessible::Property::Label(&gettext("Location"))]);
        let scroller = gtk::ScrolledWindow::builder()
            .child(&crumbs)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .hscrollbar_policy(gtk::PolicyType::External)
            .hexpand(true)
            .build();
        let entry = gtk::Entry::builder()
            .hexpand(true)
            .placeholder_text(gettext("Type a path, for example ~/Documents"))
            .build();
        entry.update_property(&[gtk::accessible::Property::Label(&gettext("Location"))]);
        let stack = gtk::Stack::builder().hhomogeneous(false).hexpand(true).build();
        stack.add_named(&scroller, Some("crumbs"));
        stack.add_named(&entry, Some("entry"));
        self.set_child(Some(&stack));
        self.set_hexpand(true);

        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = self.downgrade();
        keys.connect_key_pressed(move |_, key, _, _| {
            let Some(bar) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            match key {
                gdk::Key::Tab => {
                    bar.complete_entry();
                    glib::Propagation::Stop
                }
                gdk::Key::Escape => {
                    bar.stop_editing();
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        entry.add_controller(keys);
        let weak = self.downgrade();
        entry.connect_activate(move |_| {
            if let Some(bar) = weak.upgrade() {
                bar.submit();
            }
        });

        let _ = imp.stack.set(stack);
        let _ = imp.crumbs.set(crumbs);
        let _ = imp.entry.set(entry);
    }

    pub fn connect_navigate(&self, callback: impl Fn(&gio::File) + 'static) {
        *self.imp().on_navigate.borrow_mut() = Some(Rc::new(callback));
    }

    pub fn connect_invalid(&self, callback: impl Fn(&str) + 'static) {
        *self.imp().on_invalid.borrow_mut() = Some(Rc::new(callback));
    }

    pub fn set_location(&self, file: &gio::File) {
        *self.imp().location.borrow_mut() = Some(file.clone());
        self.rebuild_crumbs();
        if !self.is_editing() {
            if let Some(path) = file.path() {
                self.imp().entry.get().unwrap().set_text(&display_path(&path));
            }
        }
    }

    pub fn is_editing(&self) -> bool {
        self.imp().stack.get().and_then(|s| s.visible_child_name()).as_deref() == Some("entry")
    }

    pub fn start_editing(&self) {
        let imp = self.imp();
        let entry = imp.entry.get().unwrap();
        let text = imp
            .location
            .borrow()
            .as_ref()
            .and_then(|f| f.path())
            .map(|p| display_path(&p))
            .unwrap_or_default();
        entry.set_text(&text);
        imp.stack.get().unwrap().set_visible_child_name("entry");
        entry.grab_focus();
        entry.select_region(0, -1);
    }

    pub fn stop_editing(&self) {
        self.imp().stack.get().unwrap().set_visible_child_name("crumbs");
    }

    pub fn text(&self) -> String {
        self.imp().entry.get().unwrap().text().to_string()
    }

    fn base(&self) -> Option<PathBuf> {
        self.imp().location.borrow().as_ref().and_then(|f| f.path())
    }

    fn complete_entry(&self) {
        let entry = self.imp().entry.get().unwrap();
        if let Some(done) = complete(&entry.text(), self.base().as_deref()) {
            entry.set_text(&done);
            entry.set_position(-1);
        }
    }

    fn submit(&self) {
        let text = self.text();
        if text.trim().is_empty() {
            self.stop_editing();
            return;
        }
        let path = expand(&text, self.base().as_deref());
        if path.is_dir() {
            self.stop_editing();
            let callback = self.imp().on_navigate.borrow().clone();
            if let Some(callback) = callback {
                callback(&gio::File::for_path(&path));
            }
        } else {
            let callback = self.imp().on_invalid.borrow().clone();
            if let Some(callback) = callback {
                callback(&text);
            }
        }
    }

    /// Breadcrumb labels and the folder each opens, root first.
    pub fn segments(&self) -> Vec<(String, gio::File)> {
        let Some(path) = self.base() else {
            return Vec::new();
        };
        let mut current = PathBuf::from("/");
        let mut segments = vec![(gettext("Computer"), gio::File::for_path("/"))];
        let rest = path.strip_prefix(&current).map(Path::to_path_buf).unwrap_or_default();
        for part in rest.components() {
            current.push(part);
            segments.push((part.as_os_str().to_string_lossy().into_owned(), gio::File::for_path(&current)));
        }
        segments
    }

    fn rebuild_crumbs(&self) {
        let crumbs = self.imp().crumbs.get().unwrap();
        while let Some(child) = crumbs.first_child() {
            crumbs.remove(&child);
        }
        let segments = self.segments();
        let last = segments.len().saturating_sub(1);
        for (index, (label, file)) in segments.into_iter().enumerate() {
            if index > 0 {
                let separator = gtk::Label::new(Some("›"));
                separator.add_css_class("crumb-separator");
                crumbs.append(&separator);
            }
            let button = gtk::Button::with_label(&label);
            button.add_css_class("flat");
            if index == last {
                button.add_css_class("current-crumb");
            }
            let weak = self.downgrade();
            button.connect_clicked(move |_| {
                let callback = weak.upgrade().and_then(|bar| bar.imp().on_navigate.borrow().clone());
                if let Some(callback) = callback {
                    callback(&file);
                }
            });
            crumbs.append(&button);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_tree() -> PathBuf {
        let root = std::env::temp_dir().join(format!("cairn-pathbar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for dir in ["Documents", "Downloads", "Desktop", "Music", ".config"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(root.join("Docfile.txt"), "").unwrap();
        root
    }

    #[test]
    fn completes_a_unique_folder() {
        let root = temp_tree();
        let text = format!("{}/Doc", root.display());
        assert_eq!(complete(&text, None), Some(format!("{}/Documents/", root.display())));
    }

    #[test]
    fn completes_the_common_prefix_of_several() {
        let root = temp_tree();
        let text = format!("{}/D", root.display());
        // Desktop, Documents, Downloads share only "D": nothing to add.
        assert_eq!(complete(&text, None), None);
        let text = format!("{}/Do", root.display());
        assert_eq!(complete(&text, None), None); // Documents, Downloads share "Do"
        let text = format!("{}/Dow", root.display());
        assert_eq!(complete(&text, None), Some(format!("{}/Downloads/", root.display())));
    }

    #[test]
    fn relative_names_complete_against_the_base() {
        let root = temp_tree();
        assert_eq!(complete("Mu", Some(&root)), Some("Music/".into()));
        assert_eq!(complete("mu", Some(&root)), Some("Music/".into())); // case-insensitive fallback
    }

    #[test]
    fn hidden_folders_only_when_asked() {
        let root = temp_tree();
        assert_eq!(complete(".co", Some(&root)), Some(".config/".into()));
        assert_eq!(complete("", Some(&root)), None);
    }

    #[test]
    fn expand_handles_home_and_relative_paths() {
        let home = glib::home_dir();
        assert_eq!(expand("~", None), home);
        assert_eq!(expand("~/Documents", None), home.join("Documents"));
        assert_eq!(expand("/etc", None), PathBuf::from("/etc"));
        assert_eq!(expand("x", Some(Path::new("/tmp"))), PathBuf::from("/tmp/x"));
    }
}
