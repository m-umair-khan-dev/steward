//! The folder view (DESIGN.md §3.3, P1-T03, P1-T07; look per ADR 0008).
//!
//! Model chain: listing → filter (hidden files, kind chip) → sort (+ sections
//! when grouping) → multi-selection.
//!
//! Grid mode is a ListView of rows, each a section header or a run of cards,
//! so it can show sections ("Folders 3", "Files 8", "By kind 6") with
//! differently sized cards. Rows are virtualised, so huge folders stay fast.
//! List mode is a ColumnView with section headers when grouping.

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, gio, glib};

use cairn_core::sort::{self, GroupBy, SortKey};
use cairn_core::{FileEntry, Kind, LoadState};

use crate::app_data;
use crate::file_card::{self, FileCard};
use crate::folder_card::{self, FolderCard};
use crate::format;
use crate::thumbnails::Thumbnailer;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ViewMode {
    #[default]
    Grid,
    List,
}

impl ViewMode {
    pub fn id(self) -> &'static str {
        match self {
            ViewMode::Grid => "grid",
            ViewMode::List => "list",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "grid" => Some(ViewMode::Grid),
            "list" => Some(ViewMode::List),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RowSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl RowSize {
    pub fn id(self) -> &'static str {
        match self {
            RowSize::Small => "small",
            RowSize::Medium => "medium",
            RowSize::Large => "large",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "small" => Some(RowSize::Small),
            "medium" => Some(RowSize::Medium),
            "large" => Some(RowSize::Large),
            _ => None,
        }
    }

    pub fn bigger(self) -> Self {
        match self {
            RowSize::Small => RowSize::Medium,
            _ => RowSize::Large,
        }
    }

    pub fn smaller(self) -> Self {
        match self {
            RowSize::Large => RowSize::Medium,
            _ => RowSize::Small,
        }
    }

    fn list_icon(self) -> i32 {
        match self {
            RowSize::Small => 16,
            RowSize::Medium => 24,
            RowSize::Large => 32,
        }
    }
}

const GAP: i32 = 14;

type EntryCallback = Rc<dyn Fn(&FileEntry)>;
type KindCallback = Rc<dyn Fn(Kind)>;

// --- grid rows ------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub enum RowContent {
    #[default]
    Empty,
    Header { title: String, count: u32 },
    /// Positions in the sorted model; folders render as glass folders.
    Cards(Vec<u32>),
    /// Virtual kind folders: kind, file count, bytes.
    Kinds(Vec<(Kind, u32, u64)>),
}

mod row_imp {
    use super::*;

    #[derive(Default)]
    pub struct RowItem {
        pub content: RefCell<RowContent>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RowItem {
        const NAME: &'static str = "CairnGridRowItem";
        type Type = super::RowItem;
    }

    impl ObjectImpl for RowItem {}
}

glib::wrapper! {
    pub struct RowItem(ObjectSubclass<row_imp::RowItem>);
}

impl RowItem {
    fn new(content: RowContent) -> Self {
        let row: Self = glib::Object::new();
        *row.imp().content.borrow_mut() = content;
        row
    }

    fn content(&self) -> RowContent {
        self.imp().content.borrow().clone()
    }
}

mod row_widget_imp {
    use super::*;

    #[derive(Default)]
    pub struct GridRow {
        pub header: OnceCell<gtk::Label>,
        pub cards_box: OnceCell<gtk::Box>,
        pub folder_pool: RefCell<Vec<FolderCard>>,
        pub file_pool: RefCell<Vec<FileCard>>,
        /// (position, card) for the cards bound to real items.
        pub bound: RefCell<Vec<(u32, gtk::Widget)>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GridRow {
        const NAME: &'static str = "CairnGridRow";
        type Type = super::GridRow;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for GridRow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.set_orientation(gtk::Orientation::Vertical);
            let header = gtk::Label::builder().xalign(0.0).use_markup(true).visible(false).build();
            header.add_css_class("section-header");
            let cards_box = gtk::Box::new(gtk::Orientation::Horizontal, GAP);
            cards_box.add_css_class("card-row");
            obj.append(&header);
            obj.append(&cards_box);
            let _ = self.header.set(header);
            let _ = self.cards_box.set(cards_box);
        }
    }

    impl WidgetImpl for GridRow {}
    impl BoxImpl for GridRow {}
}

glib::wrapper! {
    pub struct GridRow(ObjectSubclass<row_widget_imp::GridRow>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

// --- the view ---------------------------------------------------------------

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct FolderView {
        pub store: OnceCell<gio::ListStore>,
        pub filter: OnceCell<gtk::CustomFilter>,
        pub sorter: OnceCell<gtk::CustomSorter>,
        pub section_sorter: OnceCell<gtk::CustomSorter>,
        pub sort_model: OnceCell<gtk::SortListModel>,
        pub selection: OnceCell<gtk::MultiSelection>,
        pub stack: OnceCell<gtk::Stack>,
        pub list: OnceCell<gtk::ColumnView>,
        pub list_header_factory: OnceCell<gtk::SignalListItemFactory>,
        pub grid: OnceCell<gtk::ListView>,
        pub grid_scroller: OnceCell<gtk::ScrolledWindow>,
        pub rows: OnceCell<gio::ListStore>,
        pub status: OnceCell<adw::StatusPage>,
        pub thumbnailer: OnceCell<Rc<Thumbnailer>>,
        pub show_hidden: Cell<bool>,
        pub kind_filter: Cell<Option<Kind>>,
        /// "Filter this folder": lower-case text the names must contain.
        pub name_filter: RefCell<String>,
        pub sort_key: Cell<SortKey>,
        pub descending: Cell<bool>,
        pub group: Cell<GroupBy>,
        pub mode: Cell<ViewMode>,
        pub row_size: Cell<RowSize>,
        pub state: RefCell<Option<LoadState>>,
        pub grid_rows: RefCell<Vec<GridRow>>,
        pub width: Cell<f64>,
        pub rows_pending: Cell<bool>,
        pub counts: RefCell<HashMap<String, u32>>,
        pub counting: RefCell<HashSet<String>>,
        pub on_activate: RefCell<Option<EntryCallback>>,
        pub on_open_kind: RefCell<Option<KindCallback>>,
        pub on_changed: RefCell<Vec<Rc<dyn Fn()>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FolderView {
        const NAME: &'static str = "CairnFolderView";
        type Type = super::FolderView;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for FolderView {}
    impl WidgetImpl for FolderView {}
    impl BinImpl for FolderView {}
}

glib::wrapper! {
    pub struct FolderView(ObjectSubclass<imp::FolderView>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

fn entry_of(obj: &glib::Object) -> &FileEntry {
    obj.downcast_ref::<FileEntry>().expect("model holds FileEntry objects")
}

fn header_markup(title: &str, count: u32) -> String {
    format!(
        "<b>{}</b>  <span alpha=\"55%\">{count}</span>",
        glib::markup_escape_text(title)
    )
}

impl FolderView {
    pub fn new(store: &gio::ListStore, thumbnailer: Rc<Thumbnailer>) -> Self {
        let view: Self = glib::Object::new();
        view.setup(store, thumbnailer);
        view
    }

    fn setup(&self, store: &gio::ListStore, thumbnailer: Rc<Thumbnailer>) {
        let imp = self.imp();
        let _ = imp.store.set(store.clone());
        let _ = imp.thumbnailer.set(thumbnailer);

        let weak = self.downgrade();
        let filter = gtk::CustomFilter::new(move |obj| {
            let Some(view) = weak.upgrade() else {
                return true;
            };
            let imp = view.imp();
            let entry = entry_of(obj);
            let text = imp.name_filter.borrow();
            (imp.show_hidden.get() || !entry.data().is_hidden)
                && imp.kind_filter.get().is_none_or(|kind| entry.kind() == kind)
                && (text.is_empty() || entry.display_name().to_lowercase().contains(text.as_str()))
        });
        let filter_model = gtk::FilterListModel::new(Some(store.clone()), Some(filter.clone()));

        let weak = self.downgrade();
        let sorter = gtk::CustomSorter::new(move |a, b| {
            let Some(view) = weak.upgrade() else {
                return gtk::Ordering::Equal;
            };
            let imp = view.imp();
            sort::compare(entry_of(a).data(), entry_of(b).data(), imp.sort_key.get(), imp.descending.get())
                .into()
        });
        let weak = self.downgrade();
        let section_sorter = gtk::CustomSorter::new(move |a, b| {
            let Some(view) = weak.upgrade() else {
                return gtk::Ordering::Equal;
            };
            let group = view.imp().group.get();
            let now = app_data::now();
            let sa = sort::section_of(entry_of(a).data(), group, now);
            let sb = sort::section_of(entry_of(b).data(), group, now);
            match (sa, sb) {
                (Some(x), Some(y)) => x.order.cmp(&y.order).then_with(|| x.id.cmp(&y.id)).into(),
                _ => gtk::Ordering::Equal,
            }
        });
        let sort_model = gtk::SortListModel::new(Some(filter_model), Some(sorter.clone()));
        let selection = gtk::MultiSelection::new(Some(sort_model.clone()));

        let _ = imp.filter.set(filter);
        let _ = imp.sorter.set(sorter);
        let _ = imp.section_sorter.set(section_sorter);
        let _ = imp.sort_model.set(sort_model.clone());
        let _ = imp.selection.set(selection.clone());

        let stack = gtk::Stack::new();
        stack.add_named(&self.build_grid(), Some("grid"));
        stack.add_named(&self.build_list(&selection), Some("list"));
        let status = adw::StatusPage::builder().vexpand(true).build();
        stack.add_named(&status, Some("status"));
        let spinner = gtk::Spinner::builder()
            .spinning(true)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .width_request(32)
            .height_request(32)
            .build();
        spinner.update_property(&[gtk::accessible::Property::Label(&gettext("Loading"))]);
        stack.add_named(&spinner, Some("loading"));
        stack.add_named(&gtk::Box::new(gtk::Orientation::Vertical, 0), Some("blank"));
        self.set_child(Some(&stack));
        let _ = imp.status.set(status);
        let _ = imp.stack.set(stack);

        let weak = self.downgrade();
        sort_model.connect_items_changed(move |_, _, _, _| {
            if let Some(view) = weak.upgrade() {
                view.schedule_rows();
                view.update_page();
                view.emit_changed();
            }
        });
        let weak = self.downgrade();
        selection.connect_selection_changed(move |_, _, _| {
            if let Some(view) = weak.upgrade() {
                view.refresh_selection_looks();
                view.emit_changed();
            }
        });
        self.update_page();
    }

    // --- grid ------------------------------------------------------------------

    fn build_grid(&self) -> gtk::ScrolledWindow {
        let rows = gio::ListStore::new::<RowItem>();
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().expect("ListItem");
            item.set_activatable(false);
            item.set_focusable(false);
            item.set_child(Some(&glib::Object::new::<GridRow>()));
        });
        let weak = self.downgrade();
        factory.connect_bind(move |_, item| {
            let (Some(view), Some(item)) = (weak.upgrade(), item.downcast_ref::<gtk::ListItem>()) else {
                return;
            };
            if let (Some(row), Some(widget)) =
                (item.item().and_downcast::<RowItem>(), item.child().and_downcast::<GridRow>())
            {
                view.bind_row(&widget, &row);
            }
        });
        let weak = self.downgrade();
        factory.connect_unbind(move |_, item| {
            let (Some(view), Some(item)) = (weak.upgrade(), item.downcast_ref::<gtk::ListItem>()) else {
                return;
            };
            if let Some(widget) = item.child().and_downcast::<GridRow>() {
                view.unbind_row(&widget);
            }
        });
        let grid = gtk::ListView::new(Some(gtk::NoSelection::new(Some(rows.clone()))), Some(factory));
        grid.add_css_class("grouped-grid");
        grid.set_focusable(true);
        grid.update_property(&[gtk::accessible::Property::Label(&gettext("Files"))]);
        self.add_keyboard(&grid);
        self.add_context_menu(&grid);
        let scroller = gtk::ScrolledWindow::builder()
            .child(&grid)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        let weak = self.downgrade();
        scroller.hadjustment().connect_page_size_notify(move |adjustment| {
            if let Some(view) = weak.upgrade() {
                let old = view.imp().width.replace(adjustment.page_size());
                if (old - adjustment.page_size()).abs() > 1.0 {
                    view.schedule_rows();
                }
            }
        });
        let imp = self.imp();
        let _ = imp.rows.set(rows);
        let _ = imp.grid.set(grid);
        let _ = imp.grid_scroller.set(scroller.clone());
        scroller
    }

    fn columns_for(&self, card_width: i32) -> usize {
        let width = self.imp().width.get() - 36.0; // the grid's side padding
        ((width + f64::from(GAP)) / f64::from(card_width + 12 + GAP)).floor().max(1.0) as usize
    }

    fn schedule_rows(&self) {
        let imp = self.imp();
        if imp.rows_pending.replace(true) {
            return;
        }
        let weak = self.downgrade();
        glib::idle_add_local_once(move || {
            if let Some(view) = weak.upgrade() {
                view.imp().rows_pending.set(false);
                view.rebuild_rows();
            }
        });
    }

    /// Lays the sorted items out in section headers and card rows.
    fn rebuild_rows(&self) {
        let imp = self.imp();
        let rows_store = imp.rows.get().expect("rows");
        if imp.mode.get() != ViewMode::Grid {
            rows_store.remove_all();
            return;
        }
        let size = imp.row_size.get();
        let folder_columns = self.columns_for(folder_card::shape_width(size));
        let file_columns = self.columns_for(file_card::card_width(size));
        let sort_model = self.sort_model();
        let entries: Vec<(u32, FileEntry)> = (0..sort_model.n_items())
            .filter_map(|p| sort_model.item(p).and_downcast::<FileEntry>().map(|e| (p, e)))
            .collect();
        let mut rows: Vec<RowItem> = Vec::new();
        let section = |rows: &mut Vec<RowItem>, title: String, positions: Vec<u32>, columns: usize| {
            if positions.is_empty() {
                return;
            }
            rows.push(RowItem::new(RowContent::Header { title, count: positions.len() as u32 }));
            for chunk in positions.chunks(columns) {
                rows.push(RowItem::new(RowContent::Cards(chunk.to_vec())));
            }
        };
        let folders: Vec<u32> = entries.iter().filter(|(_, e)| e.is_dir()).map(|(p, _)| *p).collect();
        let files: Vec<u32> = entries.iter().filter(|(_, e)| !e.is_dir()).map(|(p, _)| *p).collect();
        match (imp.group.get(), imp.kind_filter.get()) {
            (GroupBy::Kind, None) => {
                section(&mut rows, gettext("Folders"), folders, folder_columns);
                let mut kinds: Vec<(Kind, u32, u64)> = Vec::new();
                for (_, entry) in entries.iter().filter(|(_, e)| !e.is_dir()) {
                    match kinds.iter_mut().find(|(k, _, _)| *k == entry.kind()) {
                        Some(slot) => {
                            slot.1 += 1;
                            slot.2 += entry.data().size;
                        }
                        None => kinds.push((entry.kind(), 1, entry.data().size)),
                    }
                }
                kinds.sort_by_key(|(k, _, _)| *k);
                if !kinds.is_empty() {
                    rows.push(RowItem::new(RowContent::Header {
                        title: gettext("By kind"),
                        count: kinds.len() as u32,
                    }));
                    for chunk in kinds.chunks(folder_columns) {
                        rows.push(RowItem::new(RowContent::Kinds(chunk.to_vec())));
                    }
                }
            }
            (GroupBy::Created, _) => {
                let now = app_data::now();
                let mut current: Option<(String, String, Vec<u32>)> = None;
                for (position, entry) in &entries {
                    let Some(s) = sort::section_of(entry.data(), GroupBy::Created, now) else {
                        continue;
                    };
                    match &mut current {
                        Some((id, _, positions)) if *id == s.id => positions.push(*position),
                        _ => {
                            if let Some((_, title, positions)) = current.take() {
                                section(&mut rows, title, positions, file_columns);
                            }
                            current = Some((s.id, gettext(s.label), vec![*position]));
                        }
                    }
                }
                if let Some((_, title, positions)) = current {
                    section(&mut rows, title, positions, file_columns);
                }
            }
            (_, Some(kind)) => {
                let columns = if kind == Kind::Folder { folder_columns } else { file_columns };
                let all: Vec<u32> = entries.iter().map(|(p, _)| *p).collect();
                section(&mut rows, format::kind(kind), all, columns);
            }
            (_, None) => {
                section(&mut rows, gettext("Folders"), folders, folder_columns);
                section(&mut rows, gettext("Files"), files, file_columns);
            }
        }
        rows_store.splice(0, rows_store.n_items(), &rows);
    }

    fn bind_row(&self, widget: &GridRow, row: &RowItem) {
        let w = widget.imp();
        let header = w.header.get().expect("header");
        let cards_box = w.cards_box.get().expect("cards box");
        self.release_row(widget);
        while let Some(child) = cards_box.first_child() {
            cards_box.remove(&child);
        }
        let size = self.imp().row_size.get();
        match row.content() {
            RowContent::Empty => {
                header.set_visible(false);
                cards_box.set_visible(false);
            }
            RowContent::Header { title, count } => {
                header.set_markup(&header_markup(&title, count));
                header.set_visible(true);
                cards_box.set_visible(false);
            }
            RowContent::Cards(positions) => {
                header.set_visible(false);
                cards_box.set_visible(true);
                let sort_model = self.sort_model();
                let (mut folders_used, mut files_used) = (0, 0);
                for position in positions {
                    let Some(entry) = sort_model.item(position).and_downcast::<FileEntry>() else {
                        continue;
                    };
                    let card: gtk::Widget = if entry.is_dir() {
                        let card = self.pooled_folder(widget, folders_used);
                        folders_used += 1;
                        card.bind_entry(&entry, size);
                        self.apply_count(&card, &entry);
                        card.upcast()
                    } else {
                        let card = self.pooled_file(widget, files_used);
                        files_used += 1;
                        card.bind(&entry, size);
                        self.imp().thumbnailer.get().expect("thumbnailer").request(&entry);
                        card.upcast()
                    };
                    cards_box.append(&card);
                    w.bound.borrow_mut().push((position, card));
                }
            }
            RowContent::Kinds(kinds) => {
                header.set_visible(false);
                cards_box.set_visible(true);
                for (index, (kind, files, bytes)) in kinds.into_iter().enumerate() {
                    let card = self.pooled_folder(widget, index);
                    card.bind_kind(kind, files, bytes, size);
                    cards_box.append(&card);
                }
            }
        }
        self.imp().grid_rows.borrow_mut().push(widget.clone());
        self.refresh_row_looks(widget);
    }

    /// Unbinds the row's cards (cancelling thumbnails that have not started).
    fn release_row(&self, widget: &GridRow) {
        let thumbnailer = self.imp().thumbnailer.get().expect("thumbnailer");
        for (_, card) in widget.imp().bound.borrow_mut().drain(..) {
            if let Some(card) = card.downcast_ref::<FileCard>() {
                if let Some(entry) = card.entry() {
                    thumbnailer.cancel(&entry);
                }
                card.unbind();
            } else if let Some(card) = card.downcast_ref::<FolderCard>() {
                card.unbind();
            }
        }
    }

    fn unbind_row(&self, widget: &GridRow) {
        self.release_row(widget);
        self.imp().grid_rows.borrow_mut().retain(|w| w != widget);
    }

    fn pooled_folder(&self, widget: &GridRow, index: usize) -> FolderCard {
        let mut pool = widget.imp().folder_pool.borrow_mut();
        while pool.len() <= index {
            let card = FolderCard::default();
            self.add_card_gestures(card.upcast_ref());
            pool.push(card);
        }
        pool[index].clone()
    }

    fn pooled_file(&self, widget: &GridRow, index: usize) -> FileCard {
        let mut pool = widget.imp().file_pool.borrow_mut();
        while pool.len() <= index {
            let card = FileCard::default();
            self.add_card_gestures(card.upcast_ref());
            pool.push(card);
        }
        pool[index].clone()
    }

    fn card_target(card: &gtk::Widget) -> (Option<FileEntry>, Option<Kind>) {
        if let Some(card) = card.downcast_ref::<FileCard>() {
            (card.entry(), None)
        } else if let Some(card) = card.downcast_ref::<FolderCard>() {
            (card.entry(), card.kind())
        } else {
            (None, None)
        }
    }

    fn add_card_gestures(&self, card: &gtk::Widget) {
        let click = gtk::GestureClick::new();
        let weak = self.downgrade();
        let card_weak = card.downgrade();
        click.connect_pressed(move |gesture, n_press, _, _| {
            let (Some(view), Some(card)) = (weak.upgrade(), card_weak.upgrade()) else {
                return;
            };
            if let Some(grid) = view.imp().grid.get() {
                grid.grab_focus();
            }
            let (entry, kind) = Self::card_target(&card);
            if let Some(kind) = kind {
                view.open_kind(kind);
                return;
            }
            let Some(position) = entry.and_then(|e| view.position_of(&e)) else {
                return;
            };
            let selection = view.selection();
            if n_press >= 2 {
                view.activate_position(position);
            } else if gesture.current_event_state().contains(gdk::ModifierType::CONTROL_MASK) {
                if selection.is_selected(position) {
                    selection.unselect_item(position);
                } else {
                    selection.select_item(position, false);
                }
            } else {
                selection.select_item(position, true);
            }
        });
        card.add_controller(click);
    }

    fn refresh_row_looks(&self, widget: &GridRow) {
        let selection = self.selection();
        for (position, card) in widget.imp().bound.borrow().iter() {
            let selected = selection.is_selected(*position);
            if let Some(card) = card.downcast_ref::<FileCard>() {
                card.set_selected_look(selected);
            } else if let Some(card) = card.downcast_ref::<FolderCard>() {
                card.set_selected_look(selected);
            }
        }
    }

    fn refresh_selection_looks(&self) {
        for widget in self.imp().grid_rows.borrow().iter() {
            self.refresh_row_looks(widget);
        }
    }

    /// Item counts on glass folders: counted once per folder, in the
    /// background, then shown on every card for that folder.
    fn apply_count(&self, card: &FolderCard, entry: &FileEntry) {
        let uri = entry.data().uri();
        if let Some(count) = self.imp().counts.borrow().get(&uri) {
            card.set_count(Some(*count));
            return;
        }
        if !self.imp().counting.borrow_mut().insert(uri.clone()) {
            return;
        }
        let weak = self.downgrade();
        let file = entry.data().file.clone();
        glib::spawn_future_local(async move {
            let mut count = 0u32;
            if let Ok(enumerator) = file
                .enumerate_children_future(
                    "standard::name,standard::is-hidden",
                    gio::FileQueryInfoFlags::NONE,
                    glib::Priority::LOW,
                )
                .await
            {
                while let Ok(infos) = enumerator.next_files_future(500, glib::Priority::LOW).await {
                    if infos.is_empty() {
                        break;
                    }
                    count += infos.iter().filter(|i| !i.is_hidden()).count() as u32;
                }
            }
            let Some(view) = weak.upgrade() else {
                return;
            };
            let imp = view.imp();
            imp.counts.borrow_mut().insert(uri.clone(), count);
            imp.counting.borrow_mut().remove(&uri);
            for row in imp.grid_rows.borrow().iter() {
                for (_, card) in row.imp().bound.borrow().iter() {
                    if let Some(card) = card.downcast_ref::<FolderCard>() {
                        if card.entry().is_some_and(|e| e.data().uri() == uri) {
                            card.set_count(Some(count));
                        }
                    }
                }
            }
        });
    }

    /// Arrow keys move the selection; Return opens it.
    fn add_keyboard(&self, grid: &gtk::ListView) {
        let keys = gtk::EventControllerKey::new();
        let weak = self.downgrade();
        keys.connect_key_pressed(move |_, key, _, state| {
            let Some(view) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let n = view.sort_model().n_items();
            if n == 0 {
                return glib::Propagation::Proceed;
            }
            let selection = view.selection();
            let current = (0..n).find(|&i| selection.is_selected(i));
            let size = view.imp().row_size.get();
            let step_down = |position: u32| {
                let is_dir = view
                    .sort_model()
                    .item(position)
                    .and_downcast::<FileEntry>()
                    .is_some_and(|e| e.is_dir());
                let width =
                    if is_dir { folder_card::shape_width(size) } else { file_card::card_width(size) };
                view.columns_for(width) as i64
            };
            let target: Option<i64> = match (key, current) {
                (gdk::Key::Return | gdk::Key::KP_Enter, Some(position)) => {
                    view.activate_position(position);
                    return glib::Propagation::Stop;
                }
                (gdk::Key::Right, Some(p)) => Some(p as i64 + 1),
                (gdk::Key::Left, Some(p)) => Some(p as i64 - 1),
                (gdk::Key::Down, Some(p)) => Some(p as i64 + step_down(p)),
                (gdk::Key::Up, Some(p)) => Some(p as i64 - step_down(p)),
                (gdk::Key::Home, _) => Some(0),
                (gdk::Key::End, _) => Some(n as i64 - 1),
                (gdk::Key::Right | gdk::Key::Left | gdk::Key::Down | gdk::Key::Up, None) => Some(0),
                (gdk::Key::a, _) if state.contains(gdk::ModifierType::CONTROL_MASK) => {
                    selection.select_all();
                    return glib::Propagation::Stop;
                }
                _ => None,
            };
            match target {
                Some(t) => {
                    let t = t.clamp(0, n as i64 - 1) as u32;
                    selection.select_item(t, true);
                    view.scroll_to_position(t);
                    glib::Propagation::Stop
                }
                None => glib::Propagation::Proceed,
            }
        });
        grid.add_controller(keys);
    }

    fn scroll_to_position(&self, position: u32) {
        let imp = self.imp();
        let (Some(rows), Some(grid)) = (imp.rows.get(), imp.grid.get()) else {
            return;
        };
        for index in 0..rows.n_items() {
            if let Some(row) = rows.item(index).and_downcast::<RowItem>() {
                if matches!(row.content(), RowContent::Cards(ref p) if p.contains(&position)) {
                    grid.scroll_to(index, gtk::ListScrollFlags::NONE, None);
                    return;
                }
            }
        }
    }

    // --- list ------------------------------------------------------------------

    fn build_list(&self, selection: &gtk::MultiSelection) -> gtk::ScrolledWindow {
        let list = gtk::ColumnView::builder().model(selection).show_column_separators(false).build();
        list.add_css_class("file-list");
        list.update_property(&[gtk::accessible::Property::Label(&gettext("Files"))]);

        let weak = self.downgrade();
        let name = gtk::SignalListItemFactory::new();
        name.connect_setup(|_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().expect("ListItem");
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.append(&gtk::Image::new());
            row.append(
                &gtk::Label::builder().xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::Middle).build(),
            );
            item.set_child(Some(&row));
        });
        name.connect_bind(move |_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().expect("ListItem");
            let (Some(entry), Some(row)) = (item.item().and_downcast::<FileEntry>(), item.child()) else {
                return;
            };
            let data = entry.data();
            let size = weak.upgrade().map(|v| v.imp().row_size.get().list_icon()).unwrap_or(24);
            if let Some(image) = row.first_child().and_downcast::<gtk::Image>() {
                image.set_pixel_size(size);
                match &data.icon {
                    Some(icon) => image.set_from_gicon(icon),
                    None => image.set_icon_name(Some("text-x-generic")),
                }
            }
            if let Some(label) = row.last_child().and_downcast::<gtk::Label>() {
                label.set_label(&data.display_name);
            }
            row.update_property(&[gtk::accessible::Property::Label(&data.display_name)]);
        });
        let column = gtk::ColumnViewColumn::new(Some(&gettext("Name")), Some(name));
        column.set_expand(true);
        list.append_column(&column);

        let text_column = |title: String, numeric: bool, text: fn(&FileEntry) -> String| {
            let factory = gtk::SignalListItemFactory::new();
            factory.connect_setup(move |_, item| {
                let item = item.downcast_ref::<gtk::ListItem>().expect("ListItem");
                let label = gtk::Label::builder().xalign(if numeric { 1.0 } else { 0.0 }).build();
                label.add_css_class("dim-label");
                if numeric {
                    label.add_css_class("numeric");
                }
                item.set_child(Some(&label));
            });
            factory.connect_bind(move |_, item| {
                let item = item.downcast_ref::<gtk::ListItem>().expect("ListItem");
                if let (Some(entry), Some(label)) =
                    (item.item().and_downcast::<FileEntry>(), item.child().and_downcast::<gtk::Label>())
                {
                    label.set_label(&text(&entry));
                }
            });
            gtk::ColumnViewColumn::new(Some(&title), Some(factory))
        };
        list.append_column(&text_column(gettext("Size"), true, |e| {
            if e.is_dir() { "—".into() } else { format::size(e.data().size) }
        }));
        list.append_column(&text_column(gettext("Kind"), false, |e| format::kind(e.kind())));
        list.append_column(&text_column(gettext("Modified"), false, |e| format::date(e.data().modified)));
        list.append_column(&text_column(gettext("Created"), false, |e| format::date(e.data().created)));

        let weak = self.downgrade();
        let header = gtk::SignalListItemFactory::new();
        header.connect_setup(|_, item| {
            let header = item.downcast_ref::<gtk::ListHeader>().expect("ListHeader");
            let label = gtk::Label::builder().xalign(0.0).use_markup(true).build();
            label.add_css_class("section-header");
            header.set_child(Some(&label));
        });
        header.connect_bind(move |_, item| {
            let header = item.downcast_ref::<gtk::ListHeader>().expect("ListHeader");
            let (Some(view), Some(entry), Some(label)) = (
                weak.upgrade(),
                header.item().and_downcast::<FileEntry>(),
                header.child().and_downcast::<gtk::Label>(),
            ) else {
                return;
            };
            if let Some(section) = sort::section_of(entry.data(), view.imp().group.get(), app_data::now()) {
                label.set_markup(&header_markup(&gettext(section.label), header.n_items()));
            }
        });
        let _ = self.imp().list_header_factory.set(header);

        let weak = self.downgrade();
        list.connect_activate(move |_, position| {
            if let Some(view) = weak.upgrade() {
                view.activate_position(position);
            }
        });
        self.add_context_menu(&list);
        let _ = self.imp().list.set(list.clone());
        gtk::ScrolledWindow::builder().child(&list).build()
    }

    // --- page shown ---------------------------------------------------------

    pub fn set_load_state(&self, state: &LoadState) {
        *self.imp().state.borrow_mut() = Some(state.clone());
        if *state == LoadState::Loading {
            // Only show a spinner if loading takes a noticeable time.
            let weak = self.downgrade();
            glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
                if let Some(view) = weak.upgrade() {
                    view.update_page();
                }
            });
            self.imp().stack.get().expect("stack").set_visible_child_name("blank");
            return;
        }
        self.update_page();
    }

    pub fn page(&self) -> String {
        self.imp().stack.get().and_then(|s| s.visible_child_name()).map(|s| s.to_string()).unwrap_or_default()
    }

    pub fn status_title(&self) -> String {
        self.imp().status.get().map(|s| s.title().to_string()).unwrap_or_default()
    }

    fn update_page(&self) {
        let imp = self.imp();
        let (Some(stack), Some(status)) = (imp.stack.get(), imp.status.get()) else {
            return;
        };
        let state = imp.state.borrow().clone();
        let show_status = |icon: &str, title: String, description: String| {
            status.set_icon_name(Some(icon));
            status.set_title(&title);
            status.set_description(Some(&description));
            stack.set_visible_child_name("status");
        };
        match state {
            None | Some(LoadState::Loading) => stack.set_visible_child_name("loading"),
            Some(LoadState::PermissionDenied) => show_status(
                "action-unavailable-symbolic",
                gettext("You Can't Open This Folder"),
                gettext("You don't have permission to see what's inside."),
            ),
            Some(LoadState::NotFound) => show_status(
                "folder-symbolic",
                gettext("This Folder Is Gone"),
                gettext("It was moved or deleted, or its drive is no longer connected."),
            ),
            Some(LoadState::NotAFolder) => show_status(
                "dialog-warning-symbolic",
                gettext("This Is Not a Folder"),
                gettext("Open it from the folder that contains it."),
            ),
            Some(LoadState::Failed(message)) => {
                show_status("dialog-warning-symbolic", gettext("Couldn't Open This Folder"), message)
            }
            Some(LoadState::Ready) if self.sort_model().n_items() == 0 => {
                let searching = !imp.name_filter.borrow().is_empty();
                let (title, description) = match imp.kind_filter.get() {
                    _ if searching => (
                        gettext("No Matches"),
                        gettext("No names in this folder contain what you typed."),
                    ),
                    Some(kind) => (
                        gettext("Nothing of This Kind"),
                        format!("{} {}", gettext("No items in this folder are"), format::kind(kind)),
                    ),
                    None => (gettext("Folder Is Empty"), gettext("Nothing is in this folder yet.")),
                };
                show_status("folder-symbolic", title, description);
            }
            Some(LoadState::Ready) => {
                stack.set_visible_child_name(match imp.mode.get() {
                    ViewMode::Grid => "grid",
                    ViewMode::List => "list",
                });
            }
        }
    }

    // --- options ------------------------------------------------------------

    pub fn set_mode(&self, mode: ViewMode) {
        self.imp().mode.set(mode);
        self.schedule_rows();
        self.update_page();
    }

    pub fn mode(&self) -> ViewMode {
        self.imp().mode.get()
    }

    pub fn set_row_size(&self, size: RowSize) {
        let imp = self.imp();
        if imp.row_size.replace(size) == size {
            return;
        }
        if let Some(list) = imp.list.get() {
            let selection = self.selection();
            list.set_model(None::<&gtk::SelectionModel>);
            list.set_model(Some(&selection));
        }
        self.rebuild_rows();
    }

    pub fn row_size(&self) -> RowSize {
        self.imp().row_size.get()
    }

    pub fn set_sort(&self, key: SortKey, descending: bool) {
        let imp = self.imp();
        imp.sort_key.set(key);
        imp.descending.set(descending);
        imp.sorter.get().expect("sorter").changed(gtk::SorterChange::Different);
    }

    pub fn sort(&self) -> (SortKey, bool) {
        (self.imp().sort_key.get(), self.imp().descending.get())
    }

    pub fn set_group(&self, group: GroupBy) {
        let imp = self.imp();
        imp.group.set(group);
        let sort_model = self.sort_model();
        let list = imp.list.get().expect("list");
        if group == GroupBy::None {
            sort_model.set_section_sorter(None::<&gtk::Sorter>);
            list.set_header_factory(None::<&gtk::ListItemFactory>);
        } else {
            let section_sorter = imp.section_sorter.get().expect("section sorter");
            sort_model.set_section_sorter(Some(section_sorter));
            section_sorter.changed(gtk::SorterChange::Different);
            list.set_header_factory(imp.list_header_factory.get());
        }
        self.schedule_rows();
        self.update_page();
    }

    pub fn group(&self) -> GroupBy {
        self.imp().group.get()
    }

    pub fn set_show_hidden(&self, show: bool) {
        self.imp().show_hidden.set(show);
        self.imp().filter.get().expect("filter").changed(gtk::FilterChange::Different);
    }

    pub fn show_hidden(&self) -> bool {
        self.imp().show_hidden.get()
    }

    pub fn set_kind_filter(&self, kind: Option<Kind>) {
        self.imp().kind_filter.set(kind);
        self.imp().filter.get().expect("filter").changed(gtk::FilterChange::Different);
        self.schedule_rows();
        self.update_page();
    }

    pub fn kind_filter(&self) -> Option<Kind> {
        self.imp().kind_filter.get()
    }

    /// Shows only items whose name contains `text` (case-insensitive).
    pub fn set_name_filter(&self, text: &str) {
        let text = text.trim().to_lowercase();
        if *self.imp().name_filter.borrow() == text {
            return;
        }
        *self.imp().name_filter.borrow_mut() = text;
        self.imp().filter.get().expect("filter").changed(gtk::FilterChange::Different);
        self.schedule_rows();
        self.update_page();
    }

    pub fn name_filter(&self) -> String {
        self.imp().name_filter.borrow().clone()
    }

    fn open_kind(&self, kind: Kind) {
        let callback = self.imp().on_open_kind.borrow().clone();
        match callback {
            Some(callback) => callback(kind),
            None => self.set_kind_filter(Some(kind)),
        }
    }

    /// Called when a kind folder is opened ("Group by Kind").
    pub fn connect_open_kind(&self, callback: impl Fn(Kind) + 'static) {
        *self.imp().on_open_kind.borrow_mut() = Some(Rc::new(callback));
    }

    // --- items and selection ------------------------------------------------

    pub fn sort_model(&self) -> gtk::SortListModel {
        self.imp().sort_model.get().expect("sort model").clone()
    }

    pub fn selection(&self) -> gtk::MultiSelection {
        self.imp().selection.get().expect("selection").clone()
    }

    /// Entries in display order (after filtering and sorting).
    pub fn visible_entries(&self) -> Vec<FileEntry> {
        let model = self.sort_model();
        (0..model.n_items()).filter_map(|i| model.item(i).and_downcast::<FileEntry>()).collect()
    }

    pub fn selected_entries(&self) -> Vec<FileEntry> {
        let selection = self.selection();
        let model = self.sort_model();
        (0..model.n_items())
            .filter(|&i| selection.is_selected(i))
            .filter_map(|i| model.item(i).and_downcast::<FileEntry>())
            .collect()
    }

    pub fn select_names(&self, names: &[String]) {
        let selection = self.selection();
        selection.unselect_all();
        let model = self.sort_model();
        for i in 0..model.n_items() {
            if let Some(entry) = model.item(i).and_downcast::<FileEntry>() {
                if names.iter().any(|n| n == entry.name()) {
                    selection.select_item(i, false);
                }
            }
        }
    }

    fn position_of(&self, entry: &FileEntry) -> Option<u32> {
        let model = self.sort_model();
        (0..model.n_items()).find(|&i| model.item(i).as_ref() == Some(entry.upcast_ref()))
    }

    /// Counts and sizes per kind among the items not hidden, ignoring the chip.
    pub fn kind_totals(&self) -> Vec<(Kind, u32, u64)> {
        let imp = self.imp();
        let store = imp.store.get().expect("store");
        let mut totals: Vec<(Kind, u32, u64)> = Vec::new();
        for i in 0..store.n_items() {
            let Some(entry) = store.item(i).and_downcast::<FileEntry>() else {
                continue;
            };
            if !imp.show_hidden.get() && entry.data().is_hidden {
                continue;
            }
            match totals.iter_mut().find(|(k, _, _)| *k == entry.kind()) {
                Some(slot) => {
                    slot.1 += 1;
                    slot.2 += entry.data().size;
                }
                None => totals.push((entry.kind(), 1, entry.data().size)),
            }
        }
        totals.sort_by_key(|(k, _, _)| *k);
        totals
    }

    /// Sections in display order with their item counts (empty when not grouping).
    pub fn sections(&self) -> Vec<(String, String, u32)> {
        let group = self.group();
        let now = app_data::now();
        let mut sections: Vec<(String, String, u32)> = Vec::new();
        for entry in self.visible_entries() {
            if let Some(section) = sort::section_of(entry.data(), group, now) {
                match sections.last_mut() {
                    Some(last) if last.0 == section.id => last.2 += 1,
                    _ => sections.push((section.id, gettext(section.label), 1)),
                }
            }
        }
        sections
    }

    /// Grid rows currently built (only rows near the screen are).
    pub fn bound_rows(&self) -> usize {
        self.imp().grid_rows.borrow().len()
    }

    /// What the grid shows right now: section headers ("Folders 3"), and the
    /// bound cards as (name, has a real thumbnail, count label). Kind folders
    /// appear as "kind:<id>".
    pub fn grid_layout(&self) -> (Vec<String>, Vec<(String, bool, String)>) {
        let imp = self.imp();
        let mut headers = Vec::new();
        if let Some(rows) = imp.rows.get() {
            for i in 0..rows.n_items() {
                if let Some(RowContent::Header { title, count }) =
                    rows.item(i).and_downcast::<RowItem>().map(|r| r.content())
                {
                    headers.push(format!("{title} {count}"));
                }
            }
        }
        let mut cards = Vec::new();
        for row in imp.grid_rows.borrow().iter() {
            let cards_box = row.imp().cards_box.get().unwrap();
            let mut child = cards_box.first_child();
            while let Some(widget) = child {
                if let Some(card) = widget.downcast_ref::<FileCard>() {
                    if let Some(entry) = card.entry() {
                        cards.push((entry.name().to_string(), card.has_thumbnail(), String::new()));
                    }
                } else if let Some(card) = widget.downcast_ref::<FolderCard>() {
                    match (card.entry(), card.kind()) {
                        (Some(entry), _) => {
                            cards.push((entry.name().to_string(), false, card.count_label()))
                        }
                        (None, Some(kind)) => {
                            cards.push((format!("kind:{}", kind.id()), false, card.count_label()))
                        }
                        _ => {}
                    }
                }
                child = widget.next_sibling();
            }
        }
        (headers, cards)
    }

    fn activate_position(&self, position: u32) {
        let Some(entry) = self.sort_model().item(position).and_downcast::<FileEntry>() else {
            return;
        };
        let callback = self.imp().on_activate.borrow().clone();
        if let Some(callback) = callback {
            callback(&entry);
        }
    }

    pub fn connect_activate(&self, callback: impl Fn(&FileEntry) + 'static) {
        *self.imp().on_activate.borrow_mut() = Some(Rc::new(callback));
    }

    pub fn connect_changed(&self, callback: impl Fn() + 'static) {
        self.imp().on_changed.borrow_mut().push(Rc::new(callback));
    }

    fn emit_changed(&self) {
        let callbacks = self.imp().on_changed.borrow().clone();
        for callback in callbacks {
            callback();
        }
    }

    /// Right-click: select the item under the pointer (if not already
    /// selected), then show the item menu.
    fn add_context_menu(&self, target: &impl IsA<gtk::Widget>) {
        let click = gtk::GestureClick::builder().button(gdk::BUTTON_SECONDARY).build();
        let weak = self.downgrade();
        let target_weak = target.upcast_ref::<gtk::Widget>().downgrade();
        click.connect_pressed(move |_, _, x, y| {
            let (Some(view), Some(target)) = (weak.upgrade(), target_weak.upgrade()) else {
                return;
            };
            let picked = target.pick(x, y, gtk::PickFlags::DEFAULT);
            let entry = picked.and_then(|w| {
                w.ancestor(FileCard::static_type())
                    .and_downcast::<FileCard>()
                    .and_then(|c| c.entry())
                    .or_else(|| {
                        w.ancestor(FolderCard::static_type())
                            .and_downcast::<FolderCard>()
                            .and_then(|c| c.entry())
                    })
            });
            if let Some(position) = entry.and_then(|e| view.position_of(&e)) {
                if !view.selection().is_selected(position) {
                    view.selection().select_item(position, true);
                }
            }
            let menu = gio::Menu::new();
            menu.append(Some(&gettext("_Open")), Some("win.open-selected"));
            menu.append(Some(&gettext("Open _With…")), Some("win.open-with"));
            let popover = gtk::PopoverMenu::from_model(Some(&menu));
            popover.set_parent(&target);
            popover.set_has_arrow(false);
            popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            popover.connect_closed(|popover| {
                let popover = popover.clone();
                glib::idle_add_local_once(move || popover.unparent());
            });
            popover.popup();
        });
        target.add_controller(click);
    }
}
