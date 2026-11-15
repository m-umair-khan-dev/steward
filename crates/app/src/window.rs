//! The main window: wires the sidebar, path bar, folder view and preview pane
//! to one folder listing, and owns navigation and the window's actions.
//! Its layout is in `data/ui/window.blp`.

use std::cell::{Cell, OnceCell, RefCell};
use std::time::Instant;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};

use cairn_core::sort::{GroupBy, SortKey};
use cairn_core::store::FolderView as StoredView;
use cairn_core::{FileEntry, FolderModel, Kind, LoadState};

use crate::app_data;
use crate::folder_view::{FolderView, RowSize, ViewMode};
use crate::format;
use crate::path_bar::{self, PathBar};
use crate::preview_pane::{Overview, PreviewPane};
use crate::settings;
use crate::sidebar::Sidebar;
use crate::thumbnails::Thumbnailer;

mod imp {
    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/dev/cairn/Files/ui/window.ui")]
    pub struct CairnWindow {
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub split_view: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub sidebar_slot: TemplateChild<adw::Bin>,
        #[template_child]
        pub path_bar_slot: TemplateChild<adw::Bin>,
        #[template_child]
        pub folder_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub folder_summary: TemplateChild<gtk::Label>,
        #[template_child]
        pub chips_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub folder_view_slot: TemplateChild<adw::Bin>,
        #[template_child]
        pub preview_slot: TemplateChild<adw::Bin>,
        #[template_child]
        pub status_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub filter_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub rail_slot: TemplateChild<adw::Bin>,

        pub model: FolderModel,
        pub folder_view: OnceCell<FolderView>,
        pub sidebar: OnceCell<Sidebar>,
        pub path_bar: OnceCell<PathBar>,
        pub preview: OnceCell<PreviewPane>,
        pub back: RefCell<Vec<gio::File>>,
        pub forward: RefCell<Vec<gio::File>>,
        pub free: Cell<Option<u64>>,
        pub summary_pending: Cell<bool>,
        pub load_started: Cell<Option<Instant>>,
        pub last_load_ms: Cell<Option<u64>>,
        pub chips_signature: RefCell<String>,
        pub last_toast: RefCell<Option<String>>,
    }

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

    impl ObjectImpl for CairnWindow {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup();
        }
    }

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

/// Parses a `win.go-to` target: a URI, or a path (`~` allowed).
fn file_from_target(target: &str) -> gio::File {
    if target.contains("://") {
        gio::File::for_uri(target)
    } else {
        gio::File::for_path(path_bar::expand(target, None))
    }
}

impl CairnWindow {
    pub fn new(app: &adw::Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }

    fn setup(&self) {
        let imp = self.imp();
        let settings = settings::get();
        settings.bind("window-width", self, "default-width").build();
        settings.bind("window-height", self, "default-height").build();
        settings.bind("window-maximized", self, "maximized").build();

        let folder_view = FolderView::new(imp.model.store(), Thumbnailer::new());
        imp.folder_view_slot.set_child(Some(&folder_view));
        let sidebar = Sidebar::default();
        imp.sidebar_slot.set_child(Some(&sidebar));
        imp.rail_slot.set_child(Some(&sidebar.rail()));
        self.setup_sidebar_hover(&sidebar);
        let path_bar = PathBar::default();
        imp.path_bar_slot.set_child(Some(&path_bar));
        let preview = PreviewPane::default();
        imp.preview_slot.set_child(Some(&preview));

        let weak = self.downgrade();
        sidebar.connect_open(move |file| {
            if let Some(window) = weak.upgrade() {
                window.navigate(file);
            }
        });
        let weak = self.downgrade();
        path_bar.connect_navigate(move |file| {
            if let Some(window) = weak.upgrade() {
                window.navigate(file);
            }
        });
        let weak = self.downgrade();
        path_bar.connect_invalid(move |text| {
            if let Some(window) = weak.upgrade() {
                window.toast(&format!("{} “{text}”", gettext("No folder called")));
            }
        });
        let weak = self.downgrade();
        imp.model.connect_state_changed(move |state| {
            if let Some(window) = weak.upgrade() {
                window.on_load_state(state);
            }
        });
        let weak = self.downgrade();
        folder_view.connect_activate(move |entry| {
            if let Some(window) = weak.upgrade() {
                window.open_entry(entry);
            }
        });
        let weak = self.downgrade();
        folder_view.connect_open_kind(move |kind| {
            if let Some(window) = weak.upgrade() {
                gio::prelude::ActionGroupExt::activate_action(
                    &window,
                    "filter-kind",
                    Some(&kind.id().to_variant()),
                );
            }
        });
        let weak = self.downgrade();
        folder_view.connect_changed(move || {
            if let Some(window) = weak.upgrade() {
                window.schedule_summary();
            }
        });

        let weak = self.downgrade();
        imp.filter_entry.connect_search_changed(move |entry| {
            if let Some(window) = weak.upgrade() {
                window.folder_view().set_name_filter(&entry.text());
                window.schedule_summary();
            }
        });
        let weak = self.downgrade();
        imp.filter_entry.connect_stop_search(move |entry| {
            entry.set_text("");
            if let Some(window) = weak.upgrade() {
                window.folder_view().grab_focus();
            }
        });

        let _ = imp.folder_view.set(folder_view);
        let _ = imp.sidebar.set(sidebar);
        let _ = imp.path_bar.set(path_bar);
        let _ = imp.preview.set(preview);

        self.setup_actions();
        self.follow_settings();
        self.navigate(&gio::File::for_path(glib::home_dir()));
    }

    pub fn folder_view(&self) -> &FolderView {
        self.imp().folder_view.get().expect("folder view")
    }

    fn sidebar(&self) -> &Sidebar {
        self.imp().sidebar.get().expect("sidebar")
    }

    fn path_bar(&self) -> &PathBar {
        self.imp().path_bar.get().expect("path bar")
    }

    fn preview(&self) -> &PreviewPane {
        self.imp().preview.get().expect("preview")
    }

    pub fn toast(&self, text: &str) {
        *self.imp().last_toast.borrow_mut() = Some(text.to_string());
        self.imp().toast_overlay.add_toast(adw::Toast::new(text));
    }

    // --- settings ------------------------------------------------------------

    fn follow_settings(&self) {
        let settings = settings::get();
        let apply = glib::clone!(
            #[weak(rename_to = window)]
            self,
            move |settings: &gio::Settings, key: &str| {
                let view = window.folder_view();
                match key {
                    "show-hidden" => {
                        view.set_show_hidden(settings.boolean("show-hidden"));
                        window.schedule_summary();
                    }
                    "row-size" => view.set_row_size(
                        RowSize::from_id(&settings.string("row-size")).unwrap_or_default(),
                    ),
                    "preview-visible" => {
                        let visible = settings.boolean("preview-visible");
                        window.imp().preview_slot.set_visible(visible);
                        window.set_action_state("toggle-preview", &visible.to_variant());
                    }
                    "sidebar-visible" => {
                        // Expanded: the sidebar sits beside the content.
                        // Collapsed: an icon rail; the sidebar slides over
                        // the content while the pointer is on it (ADR 0008).
                        let expanded = settings.boolean("sidebar-visible");
                        let imp = window.imp();
                        imp.split_view.set_collapsed(!expanded);
                        imp.split_view.set_show_sidebar(expanded);
                        imp.rail_slot.set_visible(!expanded);
                        window.set_action_state("toggle-sidebar", &expanded.to_variant());
                    }
                    _ => {}
                }
            }
        );
        for key in ["show-hidden", "row-size", "preview-visible", "sidebar-visible"] {
            apply(&settings, key);
        }
        settings.connect_changed(None, move |settings, key| apply(settings, key));
    }

    fn sidebar_expanded(&self) -> bool {
        settings::get().boolean("sidebar-visible")
    }

    /// While collapsed, pointing at the rail slides the sidebar in over the
    /// content; leaving the sidebar slides it away again.
    fn setup_sidebar_hover(&self, sidebar: &Sidebar) {
        let enter = gtk::EventControllerMotion::new();
        let weak = self.downgrade();
        enter.connect_enter(move |_, _, _| {
            if let Some(window) = weak.upgrade() {
                if !window.sidebar_expanded() {
                    window.imp().split_view.set_show_sidebar(true);
                }
            }
        });
        sidebar.rail().add_controller(enter);
        let leave = gtk::EventControllerMotion::new();
        let weak = self.downgrade();
        leave.connect_leave(move |_| {
            if let Some(window) = weak.upgrade() {
                if !window.sidebar_expanded() {
                    window.imp().split_view.set_show_sidebar(false);
                }
            }
        });
        sidebar.add_controller(leave);
    }

    fn set_action_state(&self, name: &str, state: &glib::Variant) {
        if let Some(action) = self.lookup_action(name).and_downcast::<gio::SimpleAction>() {
            action.set_state(state);
        }
    }

    fn set_action_enabled(&self, name: &str, enabled: bool) {
        if let Some(action) = self.lookup_action(name).and_downcast::<gio::SimpleAction>() {
            action.set_enabled(enabled);
        }
    }

    // --- navigation ------------------------------------------------------------

    pub fn current(&self) -> Option<gio::File> {
        self.imp().model.folder()
    }

    /// Opens `folder` in this window, recording where we were for Back.
    pub fn navigate(&self, folder: &gio::File) {
        self.go(folder, true);
    }

    fn go(&self, folder: &gio::File, record: bool) {
        let imp = self.imp();
        if let Some(current) = self.current() {
            if current.equal(folder) {
                imp.model.reload();
                return;
            }
            if record {
                imp.back.borrow_mut().push(current);
                imp.forward.borrow_mut().clear();
            }
        }
        self.apply_stored_view(folder);
        imp.filter_entry.set_text("");
        if !self.sidebar_expanded() {
            imp.split_view.set_show_sidebar(false);
        }
        imp.load_started.set(Some(Instant::now()));
        imp.last_load_ms.set(None);
        imp.model.load(folder);
        self.path_bar().set_location(folder);
        self.sidebar().select_location(folder);
        imp.folder_title.set_label(&self.folder_label(folder));
        self.preview().show(None);
        self.update_navigation_actions();
        self.refresh_free_space(folder);
    }

    fn folder_label(&self, folder: &gio::File) -> String {
        match folder.path() {
            Some(path) if path == glib::home_dir() => gettext("Home"),
            Some(path) if path == std::path::Path::new("/") => gettext("Computer"),
            _ => folder.basename().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        }
    }

    fn update_navigation_actions(&self) {
        let imp = self.imp();
        self.set_action_enabled("back", !imp.back.borrow().is_empty());
        self.set_action_enabled("forward", !imp.forward.borrow().is_empty());
        self.set_action_enabled("up", self.current().and_then(|f| f.parent()).is_some());
    }

    fn go_back(&self) {
        let imp = self.imp();
        let Some(previous) = imp.back.borrow_mut().pop() else {
            return;
        };
        if let Some(current) = self.current() {
            imp.forward.borrow_mut().push(current);
        }
        self.go(&previous, false);
    }

    fn go_forward(&self) {
        let imp = self.imp();
        let Some(next) = imp.forward.borrow_mut().pop() else {
            return;
        };
        if let Some(current) = self.current() {
            imp.back.borrow_mut().push(current);
        }
        self.go(&next, false);
    }

    fn go_up(&self) {
        if let Some(parent) = self.current().and_then(|f| f.parent()) {
            self.navigate(&parent);
        }
    }

    fn refresh_free_space(&self, folder: &gio::File) {
        let weak = self.downgrade();
        let folder = folder.clone();
        glib::spawn_future_local(async move {
            let free = folder
                .query_filesystem_info_future("filesystem::free", glib::Priority::DEFAULT)
                .await
                .ok()
                .map(|info| info.attribute_uint64("filesystem::free"));
            if let Some(window) = weak.upgrade() {
                window.imp().free.set(free);
                window.schedule_summary();
            }
        });
        self.sidebar().refresh_free_space();
    }

    fn on_load_state(&self, state: &LoadState) {
        self.folder_view().set_load_state(state);
        if *state == LoadState::Ready {
            // Time from navigation to the first frame showing the listing.
            let weak = self.downgrade();
            self.folder_view().add_tick_callback(move |_, _| {
                if let Some(window) = weak.upgrade() {
                    let imp = window.imp();
                    if let Some(started) = imp.load_started.take() {
                        imp.last_load_ms.set(Some(started.elapsed().as_millis() as u64));
                    }
                }
                glib::ControlFlow::Break
            });
        }
        self.schedule_summary();
    }

    // --- per-folder view memory --------------------------------------------------

    fn apply_stored_view(&self, folder: &gio::File) {
        let stored = app_data::store().borrow().folder_view(&folder.uri()).ok().flatten().unwrap_or_default();
        let settings = settings::get();
        let mode = stored
            .view
            .as_deref()
            .and_then(ViewMode::from_id)
            .or_else(|| ViewMode::from_id(&settings.string("default-view")))
            .unwrap_or_default();
        let sort = stored.sort.as_deref().and_then(SortKey::from_id).unwrap_or_default();
        let descending = stored.descending.unwrap_or(false);
        let group = stored.group_by.as_deref().and_then(GroupBy::from_id).unwrap_or_default();
        let view = self.folder_view();
        view.set_mode(mode);
        view.set_sort(sort, descending);
        view.set_group(group);
        view.set_kind_filter(None);
        self.set_action_state("view-mode", &mode.id().to_variant());
        self.set_action_state("sort-by", &sort.id().to_variant());
        self.set_action_state("sort-descending", &descending.to_variant());
        self.set_action_state("group-by", &group.id().to_variant());
        self.set_action_state("filter-kind", &"all".to_variant());
    }

    fn store_view(&self) {
        let Some(folder) = self.current() else {
            return;
        };
        let view = self.folder_view();
        let (sort, descending) = view.sort();
        let stored = StoredView {
            view: Some(view.mode().id().into()),
            sort: Some(sort.id().into()),
            descending: Some(descending),
            group_by: Some(view.group().id().into()),
        };
        let _ = app_data::store().borrow().set_folder_view(&folder.uri(), &stored, app_data::now());
    }

    // --- summary, chips, status, preview -----------------------------------------

    fn schedule_summary(&self) {
        let imp = self.imp();
        if imp.summary_pending.replace(true) {
            return;
        }
        let weak = self.downgrade();
        glib::idle_add_local_once(move || {
            if let Some(window) = weak.upgrade() {
                window.imp().summary_pending.set(false);
                window.update_summary();
            }
        });
    }

    fn update_summary(&self) {
        let imp = self.imp();
        let view = self.folder_view();
        let totals = view.kind_totals();
        let counts: Vec<(Kind, u32)> = totals.iter().map(|(k, n, _)| (*k, *n)).collect();
        let folders: u32 = counts.iter().filter(|(k, _)| *k == Kind::Folder).map(|(_, n)| n).sum();
        let total: u32 = counts.iter().map(|(_, n)| n).sum();
        let files = total - folders;
        let bytes: u64 = totals.iter().filter(|(k, _, _)| *k != Kind::Folder).map(|(_, _, b)| b).sum();
        let ready = imp.model.state() == LoadState::Ready;
        let summary = if !ready {
            String::new()
        } else if total == 0 {
            gettext("Empty")
        } else {
            let mut parts = Vec::new();
            if folders > 0 {
                parts.push(format::folders(folders));
            }
            if files > 0 {
                parts.push(format::files(files));
            }
            let mut text = parts.join(", ");
            if files > 0 {
                text = format!("{text} · {}", format::size(bytes));
            }
            text
        };
        imp.folder_summary.set_label(&summary);
        self.update_chips(total, &counts);

        let shown = view.sort_model().n_items();
        let selected = view.selected_entries();
        let mut status = format::items(shown);
        if !selected.is_empty() {
            status = format!("{status}, {} {}", selected.len(), gettext("selected"));
        }
        if let Some(free) = imp.free.get() {
            status = format!("{status} · {} {}", format::size(free), gettext("available"));
        }
        imp.status_label.set_label(if ready { &status } else { "" });
        let overview = Overview {
            title: imp.folder_title.label().to_string(),
            bytes,
            folders,
            files,
            kinds: totals.into_iter().filter(|(k, _, _)| *k != Kind::Folder).collect(),
        };
        self.preview().set_overview(ready.then_some(overview));
        self.preview().show(if selected.len() == 1 { selected.first() } else { None });
    }

    fn update_chips(&self, total: u32, counts: &[(Kind, u32)]) {
        let imp = self.imp();
        let signature = format!("{total}:{counts:?}");
        if *imp.chips_signature.borrow() == signature {
            return;
        }
        *imp.chips_signature.borrow_mut() = signature;
        let chips = &imp.chips_box;
        while let Some(child) = chips.first_child() {
            chips.remove(&child);
        }
        if total == 0 {
            return;
        }
        let make = |id: &str, label: String, count: u32| {
            let chip = gtk::ToggleButton::builder()
                .label(format!("{label} {count}"))
                .action_name("win.filter-kind")
                .action_target(&id.to_variant())
                .build();
            chips.append(&chip);
        };
        make("all", gettext("All"), total);
        for (kind, count) in counts {
            make(kind.id(), format::kind(*kind), *count);
        }
    }

    // --- opening -------------------------------------------------------------------

    fn open_entry(&self, entry: &FileEntry) {
        if entry.is_dir() {
            self.navigate(&entry.data().file);
            return;
        }
        let uri = entry.data().uri();
        let name = entry.display_name().to_string();
        let context = gtk::prelude::WidgetExt::display(self).app_launch_context();
        let weak = self.downgrade();
        glib::spawn_future_local(async move {
            if let Err(error) = gio::AppInfo::launch_default_for_uri_future(&uri, Some(&context)).await {
                glib::g_warning!("cairn", "could not open {uri}: {error}");
                if let Some(window) = weak.upgrade() {
                    window.toast(&format!("{} “{name}”", gettext("No app could open")));
                }
            }
        });
    }

    fn open_selected(&self) {
        let selected = self.folder_view().selected_entries();
        match selected.as_slice() {
            [] => {}
            [one] => self.open_entry(one),
            many => {
                for entry in many.iter().filter(|e| !e.is_dir()) {
                    self.open_entry(entry);
                }
            }
        }
    }

    fn open_with(&self) {
        let selected = self.folder_view().selected_entries();
        let [entry] = selected.as_slice() else {
            return;
        };
        let data = entry.data();
        let apps = gio::AppInfo::all_for_type(&data.content_type);
        if apps.is_empty() {
            self.toast(&gettext("No installed app can open this kind of file"));
            return;
        }
        let list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).build();
        list.add_css_class("boxed-list");
        list.update_property(&[gtk::accessible::Property::Label(&gettext("Apps"))]);
        for app in &apps {
            let row = adw::ActionRow::builder().title(app.display_name().as_str()).activatable(true).build();
            if let Some(icon) = app.icon() {
                row.add_prefix(&gtk::Image::from_gicon(&icon));
            }
            list.append(&row);
        }
        let dialog = adw::Dialog::builder().title(gettext("Open With")).content_width(360).build();
        let content = gtk::ScrolledWindow::builder()
            .child(&list)
            .propagate_natural_height(true)
            .max_content_height(420)
            .margin_start(12)
            .margin_end(12)
            .margin_bottom(12)
            .build();
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&adw::HeaderBar::new());
        toolbar.set_content(Some(&content));
        dialog.set_child(Some(&toolbar));
        let uri = data.uri();
        let context = gtk::prelude::WidgetExt::display(self).app_launch_context();
        let dialog_weak = dialog.downgrade();
        list.connect_row_activated(move |_, row| {
            if let Some(app) = apps.get(row.index() as usize) {
                if let Err(error) = app.launch_uris(&[uri.as_str()], Some(&context)) {
                    glib::g_warning!("cairn", "could not launch: {error}");
                }
            }
            if let Some(dialog) = dialog_weak.upgrade() {
                dialog.close();
            }
        });
        dialog.present(Some(self));
    }

    // --- actions -------------------------------------------------------------------

    fn setup_actions(&self) {
        let settings = settings::get();
        let simple = |name: &str, run: fn(&CairnWindow)| {
            gio::ActionEntry::builder(name).activate(move |window: &CairnWindow, _, _| run(window)).build()
        };
        let with_string = |name: &str, run: fn(&CairnWindow, &str)| {
            gio::ActionEntry::builder(name)
                .parameter_type(Some(glib::VariantTy::STRING))
                .activate(move |window: &CairnWindow, _, param| {
                    if let Some(value) = param.and_then(|p| p.get::<String>()) {
                        run(window, &value);
                    }
                })
                .build()
        };
        let choice = |name: &str, initial: &str, apply: fn(&CairnWindow, &str)| {
            gio::ActionEntry::builder(name)
                .parameter_type(Some(glib::VariantTy::STRING))
                .state(initial.to_variant())
                .change_state(move |window: &CairnWindow, action, value| {
                    if let Some(value) = value.and_then(|v| v.get::<String>()) {
                        action.set_state(&value.to_variant());
                        apply(window, &value);
                    }
                })
                .build()
        };
        let toggle = |name: &str, initial: bool, apply: fn(&CairnWindow, bool)| {
            gio::ActionEntry::builder(name)
                .state(initial.to_variant())
                .change_state(move |window: &CairnWindow, action, value| {
                    if let Some(value) = value.and_then(|v| v.get::<bool>()) {
                        action.set_state(&value.to_variant());
                        apply(window, value);
                    }
                })
                .build()
        };

        self.add_action_entries([
            simple("back", CairnWindow::go_back),
            simple("forward", CairnWindow::go_forward),
            simple("up", CairnWindow::go_up),
            simple("home", |w| w.navigate(&gio::File::for_path(glib::home_dir()))),
            simple("reload", |w| w.imp().model.reload()),
            simple("location", |w| w.path_bar().start_editing()),
            simple("filter", |w| {
                w.imp().filter_entry.grab_focus();
            }),
            simple("open-selected", CairnWindow::open_selected),
            simple("open-with", CairnWindow::open_with),
            simple("zoom-in", |_| {
                let s = settings::get();
                let size = RowSize::from_id(&s.string("row-size")).unwrap_or_default().bigger();
                let _ = s.set_string("row-size", size.id());
            }),
            simple("zoom-out", |_| {
                let s = settings::get();
                let size = RowSize::from_id(&s.string("row-size")).unwrap_or_default().smaller();
                let _ = s.set_string("row-size", size.id());
            }),
            simple("zoom-reset", |_| {
                let _ = settings::get().set_string("row-size", RowSize::Medium.id());
            }),
            simple("sidebar-reset", |w| w.sidebar().reset()),
            with_string("go-to", |w, target| w.navigate(&file_from_target(target))),
            with_string("sidebar-hide", |w, key| w.sidebar().hide(key)),
            with_string("sidebar-move-up", |w, key| w.sidebar().move_by(key, -1)),
            with_string("sidebar-move-down", |w, key| w.sidebar().move_by(key, 1)),
            choice("view-mode", "grid", |w, id| {
                let mode = ViewMode::from_id(id).unwrap_or_default();
                w.folder_view().set_mode(mode);
                let _ = settings::get().set_string("default-view", mode.id());
                w.store_view();
            }),
            choice("sort-by", "name", |w, id| {
                let (_, descending) = w.folder_view().sort();
                w.folder_view().set_sort(SortKey::from_id(id).unwrap_or_default(), descending);
                w.store_view();
            }),
            choice("group-by", "none", |w, id| {
                w.folder_view().set_group(GroupBy::from_id(id).unwrap_or_default());
                w.store_view();
            }),
            choice("filter-kind", "all", |w, id| {
                w.folder_view().set_kind_filter(Kind::from_id(id));
                w.schedule_summary();
            }),
            toggle("sort-descending", false, |w, descending| {
                let (key, _) = w.folder_view().sort();
                w.folder_view().set_sort(key, descending);
                w.store_view();
            }),
            toggle("toggle-preview", settings.boolean("preview-visible"), |_, visible| {
                let _ = settings::get().set_boolean("preview-visible", visible);
            }),
            toggle("toggle-sidebar", settings.boolean("sidebar-visible"), |_, visible| {
                let _ = settings::get().set_boolean("sidebar-visible", visible);
            }),
        ]);
        self.add_action(&settings.create_action("show-hidden"));
    }

    // --- test state (e2e builds only) ----------------------------------------------

    #[cfg(feature = "e2e")]
    pub fn test_state(&self) -> serde_json::Value {
        use serde_json::json;
        let imp = self.imp();
        let view = self.folder_view();
        let (sort, descending) = view.sort();
        let names = |entries: Vec<FileEntry>| -> Vec<String> {
            entries.iter().map(|e| e.name().to_string()).collect()
        };
        let (headers, layout) = view.grid_layout();
        let cards: Vec<_> = layout
            .into_iter()
            .map(|(name, thumbnail, count)| json!({ "name": name, "thumbnail": thumbnail, "count": count }))
            .collect();
        let chips: Vec<_> = {
            let mut list = Vec::new();
            let mut child = imp.chips_box.first_child();
            while let Some(widget) = child {
                if let Some(button) = widget.downcast_ref::<gtk::ToggleButton>() {
                    list.push(json!({
                        "label": button.label().map(|l| l.to_string()),
                        "active": button.is_active(),
                    }));
                }
                child = widget.next_sibling();
            }
            list
        };
        let preview = self.preview();
        json!({
            "folder": {
                "uri": self.current().map(|f| f.uri().to_string()),
                "path": self.current().and_then(|f| f.path()).map(|p| p.display().to_string()),
                "title": imp.folder_title.label().to_string(),
                "summary": imp.folder_summary.label().to_string(),
                "status": imp.status_label.label().to_string(),
                "state": imp.model.state().id(),
                "page": view.page(),
                "status_title": view.status_title(),
                "view_mode": view.mode().id(),
                "row_size": view.row_size().id(),
                "sort": sort.id(),
                "descending": descending,
                "group_by": view.group().id(),
                "show_hidden": view.show_hidden(),
                "filter_kind": view.kind_filter().map(|k| k.id()).unwrap_or("all"),
                "name_filter": view.name_filter(),
                "items": names(view.visible_entries()),
                "selection": names(view.selected_entries()),
                "sections": view.sections().into_iter()
                    .map(|(id, label, count)| json!({ "id": id, "label": label, "count": count }))
                    .collect::<Vec<_>>(),
                "chips": chips,
                "bound_cards": cards.len(),
                "bound_rows": view.bound_rows(),
                "grid_headers": headers,
                "cards": cards,
                "last_load_ms": imp.last_load_ms.get(),
            },
            "navigation": {
                "can_go_back": !imp.back.borrow().is_empty(),
                "can_go_forward": !imp.forward.borrow().is_empty(),
            },
            "path_bar": {
                "editing": self.path_bar().is_editing(),
                "text": self.path_bar().text(),
                "segments": self.path_bar().segments().into_iter().map(|(label, _)| label).collect::<Vec<_>>(),
            },
            "sidebar": {
                "entries": self.sidebar().entries().iter().map(|e| json!({
                    "key": e.key,
                    "label": e.label,
                    "section": e.section,
                    "path": crate::sidebar::entry_path(e).map(|p| p.display().to_string()),
                })).collect::<Vec<_>>(),
                "hidden": self.sidebar().hidden(),
                "selected": self.sidebar().selected_key(),
                "visible": imp.split_view.shows_sidebar(),
                "expanded": self.sidebar_expanded(),
                "rail_visible": imp.rail_slot.is_visible(),
                "rail": self.sidebar().rail_labels(),
            },
            "preview": {
                "visible": imp.preview_slot.is_visible(),
                "name": preview.shown().map(|e| e.name().to_string()),
                "facts": preview.facts().into_iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>(),
                "children": preview.children(),
            },
            "last_toast": imp.last_toast.borrow().clone(),
            "dark": adw::StyleManager::default().is_dark(),
        })
    }
}
