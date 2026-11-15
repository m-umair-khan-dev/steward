//! The main window. Its layout is in `data/ui/window.blp`.

use adw::subclass::prelude::*;
use gtk::{gio, glib};

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/dev/cairn/Files/ui/window.ui")]
    pub struct CairnWindow {}

    #[glib::object_subclass]
    impl ObjectSubclass for CairnWindow {
        const NAME: &'static str = "CairnWindow";
        type Type = super::CairnWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for CairnWindow {}
    impl WidgetImpl for CairnWindow {}
    impl WindowImpl for CairnWindow {}
    impl ApplicationWindowImpl for CairnWindow {}
    impl AdwApplicationWindowImpl for CairnWindow {}
}

glib::wrapper! {
    pub struct CairnWindow(ObjectSubclass<imp::CairnWindow>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                    gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl CairnWindow {
    pub fn new(app: &adw::Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }
}
