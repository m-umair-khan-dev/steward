//! The controller: all state and logic of the window, in plain Rust (ADR 0009).
//!
//! QML never computes anything that matters: it renders the JSON this module
//! publishes (through the presenter) and sends commands back, mostly as GIO
//! actions. Engine callbacks are deferred to an idle so they never run while
//! the controller is borrowed.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use gettextrs::gettext;
use gio::prelude::*;
use serde_json::{Value, json};

use steward_core::sort::{self, GroupBy, SortKey};
use steward_core::{FileEntry, FolderModel, Kind, LoadState};

mod about;
mod agents;
mod bookmarks;
mod clean;
mod downloads;
mod git;
mod markdown;
pub use downloads::downloads_folder;
mod index;
mod installed;
mod openin;
mod ops;
mod pane;
mod peek;
mod search;
mod shelf;
mod style;
mod tidy;
mod tools;
mod welcome;
mod workspaces;

use crate::sidebar::{self, SidebarEntry};
use crate::thumbnails::Thumbnailer;
use crate::{actions, app_data, format, paths, settings, tint};

// --- view options ---------------------------------------------------------------

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
    /// Width of a glass folder card (shape plus padding).
    pub fn folder_width(self) -> i32 {
        match self {
            RowSize::Small => 132,
            RowSize::Medium => 168,
            RowSize::Large => 208,
        }
    }
    /// Width of a file card.
    pub fn file_width(self) -> i32 {
        match self {
            RowSize::Small => 196,
            RowSize::Medium => 240,
            RowSize::Large => 296,
        }
    }
}

pub const GAP: i32 = 18;

// --- what the presenter must refresh ----------------------------------------------

pub mod dirty {
    pub const FOLDER: u32 = 1;
    pub const ROWS: u32 = 2;
    pub const ITEMS: u32 = 4;
    pub const SELECTION: u32 = 8;
    pub const THUMBS: u32 = 16;
    pub const COUNTS: u32 = 32;
    pub const SIDEBAR: u32 = 64;
    pub const PREVIEW: u32 = 128;
    pub const PATH: u32 = 256;
    pub const NAV: u32 = 512;
    pub const PREFS: u32 = 1024;
    pub const OPS: u32 = 2048;
    pub const SEARCH: u32 = 4096;
    pub const PEEK: u32 = 8192;
    pub const PANE: u32 = 16384;
    pub const WORKSPACE: u32 = 32768;
    pub const TOOLS: u32 = 65536;
    pub const ALL: u32 = 131071;
}

/// One-off things for QML to do.
#[derive(Debug, Clone)]
pub enum Event {
    /// Text, and whether the toast offers Undo.
    Toast(String, bool),
    /// Text, and the action (and its parameter) its Undo button runs, for
    /// changes outside the operations journal (a hidden sidebar entry).
    ToastWithUndo(String, String, String),
    Quit,
    #[cfg_attr(not(feature = "e2e"), allow(dead_code))] // Test1.Screenshot
    Screenshot(String),
    Focus(&'static str),
    Dialog(String),
}

type Notifier = Rc<dyn Fn()>;

pub struct Controller {
    model: FolderModel,
    back: Vec<gio::File>,
    forward: Vec<gio::File>,
    pub view: ViewMode,
    pub row_size: RowSize,
    pub sort: SortKey,
    pub descending: bool,
    pub group: GroupBy,
    pub kind_filter: Option<Kind>,
    pub name_filter: String,
    pub show_hidden: bool,
    visible: Vec<FileEntry>,
    selection: HashSet<String>,
    anchor: Option<usize>,
    width: f64,
    rows: Vec<Vec<usize>>,
    sidebar: Vec<SidebarEntry>,
    hidden: Vec<String>,
    free: Option<u64>,
    home_space: Option<(u64, u64)>,
    thumbs: HashMap<String, String>,
    thumb_watch: HashSet<String>,
    counts: HashMap<String, u32>,
    counting: HashSet<String>,
    cards: HashMap<String, (i32, bool, String)>,
    bound_rows: i32,
    load_started: Option<Instant>,
    last_load_ms: Option<u64>,
    path_editing: bool,
    preview_children: Option<(String, Vec<String>)>,
    last_toast: Option<String>,
    last_report: Option<String>,
    ops: ops::Ops,
    index: index::IndexState,
    search: search::SearchState,
    peek: peek::PeekState,
    tidy: tidy::TidyState,
    downloads: downloads::DownloadsState,
    pane: pane::PaneState,
    workspace: workspaces::WorkspaceState,
    style: style::StyleState,
    tools: tools::ToolsState,
    clean: clean::CleanState,
    installed: installed::InstalledState,
    about: about::AboutState,
    welcome: welcome::WelcomeState,
    git: git::GitState,
    openin: openin::OpenInState,
    agents: agents::AgentsState,
    md: markdown::MarkdownState,
    bookmarks: bookmarks::BookmarksState,
    shelf: shelf::ShelfState,
    /// The sandbox Steward runs in, if any (P9-T01).
    sandbox: Option<steward_core::sandbox::Sandbox>,
    /// Path bar: folders whose name matches what is typed, and the one chosen.
    path_matches: Vec<PathBuf>,
    path_match: Option<usize>,
    dialog: String,
    dialog_data: Value,
    open_with_apps: Vec<gio::AppInfo>,
    window: (i32, i32, bool),
    dark: bool,
    dirty: u32,
    events: Vec<Event>,
    scheduled: bool,
    recompute_pending: bool,
    /// The load state as of the last recompute. The model's own state can run
    /// ahead of `visible` by one main-loop turn; everything shown uses this.
    state: LoadState,
    /// What the path field holds while editing.
    path_text: Option<String>,
    /// Cards on screen per file URI (see `item_visible`).
    on_screen: HashMap<String, i32>,
    /// Whether animations run (DESIGN.md §3.5), and how many cards are in flight.
    motion: bool,
    flying: i32,
    interface: Option<gio::Settings>,
    notifier: Option<Notifier>,
    thumbnailer: Rc<Thumbnailer>,
}

thread_local! {
    static CONTROLLER: RefCell<Option<Controller>> = const { RefCell::new(None) };
}

/// Runs `f` with the controller. Panics if called re-entrantly; engine
/// callbacks therefore go through [`later`].
pub fn with<R>(f: impl FnOnce(&mut Controller) -> R) -> R {
    CONTROLLER.with(|cell| {
        let mut slot = cell.borrow_mut();
        let controller = slot.as_mut().expect("controller is initialised");
        let result = f(controller);
        controller.flush();
        result
    })
}

/// Runs `f` with the controller on the next main-loop iteration.
pub fn later(f: impl FnOnce(&mut Controller) + 'static) {
    glib::idle_add_local_once(move || with(f));
}

/// Creates the controller and opens the home folder. `notifier` asks the
/// presenter to refresh (it is called at most once per batch of changes).
pub fn init(notifier: impl Fn() + 'static) {
    let settings = settings::get();
    let controller = Controller {
        model: FolderModel::new(),
        back: Vec::new(),
        forward: Vec::new(),
        view: ViewMode::from_id(&settings.string("default-view")).unwrap_or_default(),
        row_size: RowSize::from_id(&settings.string("row-size")).unwrap_or_default(),
        sort: SortKey::Name,
        descending: false,
        group: GroupBy::None,
        kind_filter: None,
        name_filter: String::new(),
        show_hidden: settings.boolean("show-hidden"),
        visible: Vec::new(),
        selection: HashSet::new(),
        anchor: None,
        width: 900.0,
        rows: Vec::new(),
        sidebar: Vec::new(),
        hidden: Vec::new(),
        free: None,
        home_space: None,
        thumbs: HashMap::new(),
        thumb_watch: HashSet::new(),
        counts: HashMap::new(),
        counting: HashSet::new(),
        cards: HashMap::new(),
        bound_rows: 0,
        load_started: None,
        last_load_ms: None,
        path_editing: false,
        preview_children: None,
        last_toast: None,
        last_report: None,
        ops: ops::Ops::default(),
        index: index::IndexState::default(),
        search: search::SearchState::default(),
        peek: peek::PeekState::default(),
        tidy: tidy::TidyState::default(),
        downloads: downloads::DownloadsState::default(),
        pane: pane::PaneState::default(),
        workspace: workspaces::WorkspaceState::default(),
        style: style::StyleState::default(),
        tools: tools::ToolsState::default(),
        clean: clean::CleanState::default(),
        installed: installed::InstalledState::default(),
        about: about::AboutState::default(),
        welcome: welcome::WelcomeState::default(),
        git: git::GitState::default(),
        openin: openin::OpenInState::default(),
        agents: agents::AgentsState::default(),
        md: markdown::MarkdownState::default(),
        bookmarks: bookmarks::BookmarksState::default(),
        shelf: shelf::ShelfState::default(),
        sandbox: detect_sandbox(),
        path_matches: Vec::new(),
        path_match: None,
        dialog: String::new(),
        dialog_data: Value::Null,
        open_with_apps: Vec::new(),
        window: (0, 0, false),
        dark: false,
        dirty: dirty::ALL,
        events: Vec::new(),
        scheduled: false,
        recompute_pending: false,
        state: LoadState::Loading,
        path_text: None,
        on_screen: HashMap::new(),
        motion: false,
        flying: 0,
        interface: interface_settings(),
        notifier: Some(Rc::new(notifier)),
        thumbnailer: Thumbnailer::new(),
    };
    let model = controller.model.clone();
    CONTROLLER.with(|cell| *cell.borrow_mut() = Some(controller));

    model.connect_state_changed(|state| {
        let state = state.clone();
        later(move |c| c.on_load_state(&state));
    });
    model
        .store()
        .connect_items_changed(|_, _, _, _| schedule_recompute());
    for signal in ["mount-added", "mount-removed", "mount-changed"] {
        gio::VolumeMonitor::get().connect_local(signal, false, |_| {
            later(|c| c.reload_sidebar());
            None
        });
    }
    with(|c| {
        c.motion = c.motion_wanted();
        if let Some(interface) = &c.interface {
            // GNOME's dark mode: QML uses it where Qt cannot report it (Qt 6.4).
            interface.connect_changed(Some("color-scheme"), |_, _| {
                later(|c| c.mark(dirty::PREFS));
            });
            // The accent chosen in Ubuntu's Settings, live.
            for key in ["accent-color", "gtk-theme"] {
                interface.connect_changed(Some(key), |_, _| later(|c| c.mark(dirty::PREFS)));
            }
            interface.connect_changed(Some("enable-animations"), |_, _| {
                later(|c| {
                    c.motion = c.motion_wanted();
                    c.mark(dirty::PREFS);
                });
            });
        }
    });
    settings.connect_changed(None, |_, key| {
        let key = key.to_string();
        later(move |c| c.on_setting(&key));
    });
    with(|c| {
        ops::start(c);
        index::start(c);
        c.reload_sidebar();
        c.bookmarks_load();
        // The Shelf's key matches the setting (P8-T02).
        crate::desktop::apply_shelf_shortcut();
        crate::desktop::apply_open_at_login();
        crate::tray::show(settings::get().boolean("tray-icon"));
        c.refresh_home_space();
        // Dual pane comes back with both its folders (E4-10).
        let s = settings::get();
        let dual = s.boolean("dual-pane");
        let left = std::path::PathBuf::from(s.string("pane-left").as_str());
        let right = std::path::PathBuf::from(s.string("pane-right").as_str());
        let start = if dual && left.is_dir() {
            left
        } else {
            glib::home_dir()
        };
        c.navigate(&gio::File::for_path(start), false);
        if dual {
            c.set_dual_pane(true, right.is_dir().then_some(right));
        }
        c.first_run_check();
        if settings::get().boolean("default-file-manager") {
            crate::file_manager::serve(true);
        }
    });
}

fn schedule_recompute() {
    let pending = CONTROLLER.with(|cell| {
        cell.try_borrow_mut().ok().and_then(|mut slot| {
            slot.as_mut()
                .map(|c| std::mem::replace(&mut c.recompute_pending, true))
        })
    });
    if pending != Some(true) {
        later(|c| {
            c.recompute_pending = false;
            c.recompute();
        });
    }
}

impl Controller {
    // --- refresh plumbing --------------------------------------------------------

    fn mark(&mut self, flags: u32) {
        self.dirty |= flags;
    }

    /// A short message at the bottom of the window.
    pub fn toast(&mut self, text: &str) {
        self.emit(Event::Toast(text.to_string(), false));
    }

    fn emit(&mut self, event: Event) {
        if let Event::Toast(text, _) | Event::ToastWithUndo(text, _, _) = &event {
            self.last_toast = Some(text.clone());
        }
        self.events.push(event);
    }

    /// Asks the presenter to refresh if anything changed.
    fn flush(&mut self) {
        if (self.dirty != 0 || !self.events.is_empty()) && !self.scheduled {
            self.scheduled = true;
            if let Some(notifier) = self.notifier.clone() {
                notifier();
            }
        }
    }

    /// Called by the presenter: what changed since the last refresh.
    pub fn take_changes(&mut self) -> (u32, Vec<Event>) {
        self.scheduled = false;
        (
            std::mem::take(&mut self.dirty),
            std::mem::take(&mut self.events),
        )
    }

    // --- navigation ----------------------------------------------------------------

    pub fn current(&self) -> Option<gio::File> {
        self.model.folder()
    }

    pub fn navigate(&mut self, folder: &gio::File, record: bool) {
        self.close_tool();
        // Opening the Trash before anything was ever trashed: make it, so
        // the view shows an empty Trash rather than a missing folder.
        let trash = steward_core::ops::Trash::new(steward_core::ops::Trash::default_home());
        if folder.path().as_deref() == Some(trash.files_dir().as_path())
            && !folder.query_exists(gio::Cancellable::NONE)
        {
            let _ = trash.ensure();
        }
        if let Some(current) = self.current() {
            if current.equal(folder) {
                self.model.reload();
                return;
            }
            if record {
                self.back.push(current);
                self.forward.clear();
            }
        }
        self.apply_stored_view(folder);
        self.name_filter.clear();
        self.kind_filter = None;
        self.selection.clear();
        self.anchor = None;
        self.preview_children = None;
        self.path_editing = false;
        self.ops.renaming = None;
        self.load_started = Some(Instant::now());
        self.last_load_ms = None;
        self.state = LoadState::Loading;
        self.model.load(folder);
        self.visible.clear();
        self.refresh_free_space(folder);
        self.refresh_git();
        actions::set_state("filter-kind", &"all".to_variant());
        actions::set_enabled("back", !self.back.is_empty());
        actions::set_enabled("forward", !self.forward.is_empty());
        actions::set_enabled("up", folder.parent().is_some());
        self.workspace.open = None;
        self.forget_looks();
        self.mark(dirty::ALL & !dirty::PREFS);
        if self.pane.open {
            self.remember_panes();
        }
    }

    pub fn go_back(&mut self) {
        // Back from a tool page returns to the folder under it.
        if !self.tools.page.is_empty() {
            self.close_tool();
            return;
        }
        if let Some(previous) = self.back.pop() {
            if let Some(current) = self.current() {
                self.forward.push(current);
            }
            self.navigate(&previous, false);
        }
    }

    pub fn go_forward(&mut self) {
        if let Some(next) = self.forward.pop() {
            if let Some(current) = self.current() {
                self.back.push(current);
            }
            self.navigate(&next, false);
        }
    }

    pub fn go_up(&mut self) {
        if let Some(parent) = self.current().and_then(|f| f.parent()) {
            self.navigate(&parent, true);
        }
    }

    pub fn go_to(&mut self, target: &str) {
        let file = if target.contains("://") {
            gio::File::for_uri(target)
        } else {
            gio::File::for_path(paths::expand(target, None))
        };
        // A file (from the command line or ShowItems): its folder, with it selected.
        if file.query_file_type(gio::FileQueryInfoFlags::NONE, gio::Cancellable::NONE)
            == gio::FileType::Regular
            && let Some(parent) = file.parent()
        {
            self.navigate(&parent, true);
            self.select_when_shown(&file.uri());
            return;
        }
        self.navigate(&file, true);
    }

    pub fn reload(&mut self) {
        self.model.reload();
    }

    fn on_load_state(&mut self, state: &LoadState) {
        self.state = state.clone();
        if *state == LoadState::Ready {
            self.refresh_suggestions();
            self.recompute();
            if let Some(started) = self.load_started.take() {
                self.last_load_ms = Some(started.elapsed().as_millis() as u64);
            }
        }
        self.mark(dirty::FOLDER | dirty::ROWS | dirty::ITEMS | dirty::PREVIEW);
    }

    fn refresh_free_space(&self, folder: &gio::File) {
        let folder = folder.clone();
        glib::spawn_future_local(async move {
            let free = folder
                .query_filesystem_info_future("filesystem::free", glib::Priority::DEFAULT)
                .await
                .ok()
                .map(|info| info.attribute_uint64("filesystem::free"));
            later(move |c| {
                c.free = free;
                c.mark(dirty::FOLDER);
            });
        });
    }

    fn refresh_home_space(&self) {
        let home = gio::File::for_path(glib::home_dir());
        glib::spawn_future_local(async move {
            let Ok(info) = home
                .query_filesystem_info_future(
                    "filesystem::free,filesystem::size",
                    glib::Priority::DEFAULT,
                )
                .await
            else {
                return;
            };
            let space = (
                info.attribute_uint64("filesystem::free"),
                info.attribute_uint64("filesystem::size"),
            );
            later(move |c| {
                c.home_space = Some(space);
                c.mark(dirty::SIDEBAR);
            });
        });
    }

    // --- per-folder view memory ------------------------------------------------------

    fn apply_stored_view(&mut self, folder: &gio::File) {
        let stored = app_data::store()
            .borrow()
            .folder_view(&folder.uri())
            .ok()
            .flatten()
            .unwrap_or_default();
        let settings = settings::get();
        self.view = stored
            .view
            .as_deref()
            .and_then(ViewMode::from_id)
            .or_else(|| ViewMode::from_id(&settings.string("default-view")))
            .unwrap_or_default();
        self.sort = stored
            .sort
            .as_deref()
            .and_then(SortKey::from_id)
            .unwrap_or_default();
        self.descending = stored.descending.unwrap_or(false);
        self.group = stored
            .group_by
            .as_deref()
            .and_then(GroupBy::from_id)
            .unwrap_or_default();
        actions::set_state("view-mode", &self.view.id().to_variant());
        actions::set_state("sort-by", &self.sort.id().to_variant());
        actions::set_state("sort-descending", &self.descending.to_variant());
        actions::set_state("group-by", &self.group.id().to_variant());
    }

    fn store_view(&self) {
        let Some(folder) = self.current() else {
            return;
        };
        let stored = steward_core::store::FolderView {
            view: Some(self.view.id().into()),
            sort: Some(self.sort.id().into()),
            descending: Some(self.descending),
            group_by: Some(self.group.id().into()),
        };
        let _ = app_data::store()
            .borrow()
            .set_folder_view(&folder.uri(), &stored, app_data::now());
    }

    pub fn set_view(&mut self, view: ViewMode) {
        self.view = view;
        let _ = settings::get().set_string("default-view", view.id());
        self.store_view();
        self.mark(dirty::FOLDER | dirty::ROWS | dirty::ITEMS);
    }

    pub fn set_sort(&mut self, key: SortKey, descending: bool) {
        self.sort = key;
        self.descending = descending;
        self.store_view();
        self.recompute();
    }

    pub fn set_group(&mut self, group: GroupBy) {
        self.group = group;
        self.store_view();
        self.recompute();
    }

    pub fn set_kind_filter(&mut self, kind: Option<Kind>) {
        self.kind_filter = kind;
        actions::set_state(
            "filter-kind",
            &kind.map(|k| k.id()).unwrap_or("all").to_variant(),
        );
        self.recompute();
    }

    pub fn set_name_filter(&mut self, text: &str) {
        let text = text.trim().to_lowercase();
        if self.name_filter != text {
            self.name_filter = text;
            self.recompute();
        }
    }

    fn on_setting(&mut self, key: &str) {
        let settings = settings::get();
        match key {
            "show-hidden" => {
                self.show_hidden = settings.boolean("show-hidden");
                self.recompute();
            }
            "row-size" => {
                self.row_size = RowSize::from_id(&settings.string("row-size")).unwrap_or_default();
                self.mark(dirty::ROWS | dirty::FOLDER);
            }
            "preview-visible" => {
                actions::set_state("toggle-preview", &settings.boolean(key).to_variant())
            }
            "sidebar-visible" => {
                actions::set_state("toggle-sidebar", &settings.boolean(key).to_variant())
            }
            "bookmark-view" | "bookmark-fetch" => self.mark(dirty::TOOLS),
            "tray-icon" => crate::tray::show(settings.boolean(key)),
            "open-at-login" => crate::desktop::apply_open_at_login(),
            "shelf-shortcut-enabled" | "shelf-shortcut" => {
                if !crate::desktop::apply_shelf_shortcut()
                    && settings.boolean("shelf-shortcut-enabled")
                {
                    self.toast(&gettext(
                        "This desktop has no custom shortcuts; open the Shelf with “steward shelf”",
                    ));
                }
            }
            "shortcuts" => {} // PREFS, below, republishes the shortcuts too
            "downloads-folder" => {
                self.reload_sidebar();
                self.refresh_suggestions();
            }
            "default-file-manager" => {
                let on = settings.boolean(key);
                crate::file_manager::serve(on);
                if !crate::file_manager::set_default_for_folders(on) && on {
                    self.emit(Event::Toast(
                        gettext("Steward answers other apps now; folders open in it once Steward is installed"),
                        false,
                    ));
                }
            }
            _ => {}
        }
        self.mark(dirty::PREFS | dirty::PREVIEW | dirty::SIDEBAR);
    }

    // --- the visible list --------------------------------------------------------------

    /// Filters and sorts the listing into the visible list.
    pub fn recompute(&mut self) {
        let store = self.model.store();
        let group = self.group;
        let now = app_data::now();
        let mut visible: Vec<FileEntry> = (0..store.n_items())
            .filter_map(|i| store.item(i).and_downcast::<FileEntry>())
            .filter(|e| self.show_hidden || !e.data().is_hidden)
            .filter(|e| self.kind_filter.is_none_or(|k| e.kind() == k))
            .filter(|e| {
                self.name_filter.is_empty()
                    || e.display_name().to_lowercase().contains(&self.name_filter)
            })
            .collect();
        visible.sort_by(|a, b| {
            let section = || match (
                sort::section_of(a.data(), group, now),
                sort::section_of(b.data(), group, now),
            ) {
                (Some(x), Some(y)) => x.order.cmp(&y.order),
                _ => std::cmp::Ordering::Equal,
            };
            section().then_with(|| sort::compare(a.data(), b.data(), self.sort, self.descending))
        });
        for entry in &visible {
            if let Some(path) = entry.thumbnail_path() {
                self.thumbs.entry(entry.data().uri()).or_insert(path);
            }
        }
        // Drop selected items that are gone, but only once the listing is
        // complete: while a folder reloads, the list is briefly empty and the
        // selection must survive it.
        if !matches!(self.model.state(), LoadState::Loading) {
            let uris: HashSet<String> = visible.iter().map(|e| e.data().uri()).collect();
            self.selection.retain(|uri| uris.contains(uri));
        }
        self.visible = visible;
        self.apply_pending_select();
        self.mark(
            dirty::FOLDER
                | dirty::ROWS
                | dirty::ITEMS
                | dirty::SELECTION
                | dirty::PREVIEW
                | dirty::THUMBS,
        );
    }

    fn entry_at(&self, position: usize) -> Option<&FileEntry> {
        self.visible.get(position)
    }

    fn kind_totals(&self) -> Vec<(Kind, u32, u64)> {
        let store = self.model.store();
        let mut totals: Vec<(Kind, u32, u64)> = Vec::new();
        for entry in (0..store.n_items()).filter_map(|i| store.item(i).and_downcast::<FileEntry>())
        {
            if !self.show_hidden && entry.data().is_hidden {
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

    // --- selection ------------------------------------------------------------------------

    pub fn selected(&self) -> Vec<&FileEntry> {
        self.visible
            .iter()
            .filter(|e| self.selection.contains(&e.data().uri()))
            .collect()
    }

    /// mode: 0 replace, 1 toggle (Ctrl), 2 range (Shift).
    pub fn pick(&mut self, position: usize, mode: i32) {
        self.pane_activate(false);
        let Some(uri) = self.entry_at(position).map(|e| e.data().uri()) else {
            if mode == 0 {
                self.selection.clear();
                self.mark(dirty::SELECTION | dirty::PREVIEW | dirty::FOLDER);
            }
            return;
        };
        match mode {
            1 => {
                if !self.selection.remove(&uri) {
                    self.selection.insert(uri);
                }
            }
            2 => {
                let anchor = self.anchor.unwrap_or(position);
                let (a, b) = if anchor <= position {
                    (anchor, position)
                } else {
                    (position, anchor)
                };
                self.selection = self.visible[a..=b.min(self.visible.len() - 1)]
                    .iter()
                    .map(|e| e.data().uri())
                    .collect();
            }
            _ => {
                self.selection.clear();
                self.selection.insert(uri);
            }
        }
        if mode != 2 {
            self.anchor = Some(position);
        }
        self.on_selection_changed();
    }

    pub fn select_all(&mut self) {
        self.selection = self.visible.iter().map(|e| e.data().uri()).collect();
        self.on_selection_changed();
    }

    #[cfg(feature = "e2e")]
    pub fn select_names(&mut self, names: &[String]) {
        self.selection = self
            .visible
            .iter()
            .filter(|e| names.iter().any(|n| n == e.name()))
            .map(|e| e.data().uri())
            .collect();
        self.anchor = self
            .visible
            .iter()
            .position(|e| self.selection.contains(&e.data().uri()));
        self.on_selection_changed();
    }

    /// Arrow-key movement in the grid or list.
    pub fn move_selection(&mut self, key: &str, extend: bool) {
        if self.visible.is_empty() {
            return;
        }
        let current = self.anchor.filter(|&p| p < self.visible.len());
        let columns = |position: usize| -> i64 {
            self.rows
                .iter()
                .find(|row| row.contains(&position))
                .map(|row| row.len() as i64)
                .unwrap_or(1)
        };
        let target: i64 = match (key, current) {
            (_, None) => 0,
            ("right", Some(p)) => p as i64 + 1,
            ("left", Some(p)) => p as i64 - 1,
            ("down", Some(p)) => {
                p as i64
                    + if self.view == ViewMode::Grid {
                        columns(p).max(1)
                    } else {
                        1
                    }
            }
            ("up", Some(p)) => {
                p as i64
                    - if self.view == ViewMode::Grid {
                        columns(p).max(1)
                    } else {
                        1
                    }
            }
            ("home", _) => 0,
            ("end", _) => self.visible.len() as i64 - 1,
            (_, Some(p)) => p as i64,
        };
        let target = target.clamp(0, self.visible.len() as i64 - 1) as usize;
        self.pick(target, if extend { 2 } else { 0 });
    }

    fn on_selection_changed(&mut self) {
        self.preview_children = None;
        let selected = self.selected();
        if let [entry] = selected.as_slice()
            && entry.is_dir()
        {
            let file = entry.data().file.clone();
            let uri = entry.data().uri();
            glib::spawn_future_local(async move {
                let names = list_names(&file).await;
                later(move |c| {
                    c.preview_children = Some((uri, names));
                    c.mark(dirty::PREVIEW);
                });
            });
        }
        self.mark(dirty::SELECTION | dirty::PREVIEW | dirty::FOLDER);
    }

    // --- opening -------------------------------------------------------------------------------

    pub fn open_position(&mut self, position: usize) {
        if let Some(entry) = self.entry_at(position).cloned() {
            self.open_entry(&entry);
        }
    }

    fn open_entry(&mut self, entry: &FileEntry) {
        if entry.is_dir() {
            self.navigate(&entry.data().file, true);
            return;
        }
        if let Some(path) = entry
            .data()
            .file
            .path()
            .filter(|p| markdown::is_markdown(p))
            && self.md_open(&path)
        {
            return;
        }
        let uri = entry.data().uri();
        let name = entry.display_name().to_string();
        glib::spawn_future_local(async move {
            if let Err(error) =
                gio::AppInfo::launch_default_for_uri_future(&uri, gio::AppLaunchContext::NONE).await
            {
                glib::g_warning!("steward", "could not open {uri}: {error}");
                later(move |c| {
                    c.emit(Event::Toast(
                        format!("{} “{name}”", gettext("No app could open")),
                        false,
                    ))
                });
            }
        });
    }

    pub fn open_selected(&mut self) {
        let selected: Vec<FileEntry> = self.selected().into_iter().cloned().collect();
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

    pub fn open_with(&mut self) {
        let selected: Vec<FileEntry> = self.selected().into_iter().cloned().collect();
        let [entry] = selected.as_slice() else {
            return;
        };
        let apps = gio::AppInfo::all_for_type(&entry.data().content_type);
        if apps.is_empty() {
            self.emit(Event::Toast(
                gettext("No installed app can open this kind of file"),
                false,
            ));
            return;
        }
        self.dialog_data = json!({
            "file": entry.display_name(),
            "apps": apps.iter().map(|a| a.display_name().to_string()).collect::<Vec<_>>(),
        });
        self.open_with_apps = apps;
        self.show_dialog("open-with");
    }

    pub fn launch_app(&mut self, index: usize) {
        let uri = self.selected().first().map(|e| e.data().uri());
        if let (Some(app), Some(uri)) = (self.open_with_apps.get(index), uri)
            && let Err(error) = app.launch_uris(&[uri.as_str()], gio::AppLaunchContext::NONE)
        {
            glib::g_warning!("steward", "could not launch: {error}");
        }
        self.close_dialog();
    }

    // --- dialogs, path bar, events from QML --------------------------------------------------------

    pub fn show_dialog(&mut self, kind: &str) {
        self.dialog = kind.to_string();
        self.emit(Event::Dialog(kind.to_string()));
    }

    pub fn close_dialog(&mut self) {
        // The welcome and What's New show once, however they are closed.
        let s = settings::get();
        match self.dialog.as_str() {
            "welcome" => {
                let _ = s.set_boolean("welcome-shown", true);
                let _ = s.set_string("last-version", env!("CARGO_PKG_VERSION"));
            }
            "whats-new" => {
                let _ = s.set_string("last-version", env!("CARGO_PKG_VERSION"));
            }
            _ => {}
        }
        self.dialog.clear();
        self.emit(Event::Dialog(String::new()));
    }

    pub fn dialog_shown(&mut self, kind: &str) {
        self.dialog = kind.to_string();
    }

    /// The last "Report a Bug" address opened (tests read it).
    pub fn reported(&mut self, url: String) {
        self.last_report = Some(url);
    }

    pub fn dialog_data(&self) -> &Value {
        &self.dialog_data
    }

    pub fn start_path_edit(&mut self) {
        self.path_editing = true;
        self.path_text = None;
        self.mark(dirty::PATH);
        self.emit(Event::Focus("path"));
    }

    pub fn stop_path_edit(&mut self) {
        self.path_editing = false;
        self.path_text = None;
        self.path_matches.clear();
        self.path_match = None;
        self.mark(dirty::PATH);
    }

    pub fn path_typed(&mut self, text: &str) {
        if self.path_editing {
            self.path_text = Some(text.to_string());
            // A folder's name, not a path: offer every folder with that name
            // and where it is (P3-T10).
            let name = text.trim();
            self.path_matches = if name.len() >= 2 && !name.contains('/') && !name.starts_with('~')
            {
                let running = self.index.running;
                self.index
                    .reader()
                    .filter(|_| running)
                    .map(|r| {
                        r.folders_named(name, 8)
                            .into_iter()
                            .map(|h| h.path)
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            self.path_match = None;
            self.mark(dirty::PATH);
        }
    }

    /// Down and Up in the path field move through the offered folders.
    pub fn path_match_move(&mut self, key: &str) {
        if self.path_matches.is_empty() {
            return;
        }
        let last = self.path_matches.len() - 1;
        self.path_match = Some(match (key, self.path_match) {
            ("down", None) => 0,
            ("down", Some(i)) => (i + 1).min(last),
            ("up", Some(i)) => i.saturating_sub(1),
            _ => 0,
        });
        self.mark(dirty::PATH);
    }

    pub fn path_match_go(&mut self, index: usize) {
        if let Some(path) = self.path_matches.get(index).cloned() {
            self.path_matches.clear();
            self.path_match = None;
            self.path_editing = false;
            self.path_text = None;
            self.navigate(&gio::File::for_path(&path), true);
            self.mark(dirty::PATH);
        }
    }

    pub fn complete_path(&self, text: &str) -> String {
        let base = self.current().and_then(|f| f.path());
        paths::complete(text, base.as_deref()).unwrap_or_else(|| text.to_string())
    }

    pub fn submit_path(&mut self, text: &str) {
        if text.trim().is_empty() {
            self.stop_path_edit();
            return;
        }
        // "code .", "zed src", "terminal" (P7-T03).
        if self.path_command(text) {
            self.stop_path_edit();
            return;
        }
        if let Some(chosen) = self.path_match {
            self.path_match_go(chosen);
            return;
        }
        let base = self.current().and_then(|f| f.path());
        let path = paths::expand(text, base.as_deref());
        if !path.is_dir() && self.path_matches.len() == 1 {
            self.path_match_go(0);
            return;
        }
        if !path.is_dir() && self.path_matches.len() > 1 {
            // Several folders have that name: the list stays open to choose.
            return;
        }
        if path.is_dir() {
            self.path_editing = false;
            self.path_text = None;
            self.navigate(&gio::File::for_path(&path), true);
        } else {
            self.emit(Event::Toast(
                format!("{} “{}”", gettext("No folder called"), text.trim()),
                false,
            ));
        }
        self.mark(dirty::PATH);
    }

    pub fn focus_filter(&mut self) {
        self.emit(Event::Focus("filter"));
    }

    #[cfg(feature = "e2e")]
    pub fn request_screenshot(&mut self, path: &str) {
        self.emit(Event::Screenshot(path.to_string()));
    }

    pub fn quit(&mut self) {
        self.emit(Event::Quit);
    }

    /// Why a feature that needs the host is not here (P9-T01), or None.
    pub fn sandbox_note(&self, needs: steward_core::sandbox::Needs) -> Option<String> {
        if steward_core::sandbox::available(self.sandbox, needs) {
            return None;
        }
        let name = self.sandbox.map(|s| s.name()).unwrap_or("a sandbox");
        let why = match needs {
            steward_core::sandbox::Needs::PackageLists => {
                gettext("which keeps it from reading the system’s list of installed software.")
            }
            steward_core::sandbox::Needs::PackageDatabase => {
                gettext("which keeps it from seeing which packages are installed.")
            }
            steward_core::sandbox::Needs::AppLists => {
                gettext("which keeps it from seeing which apps are installed.")
            }
            steward_core::sandbox::Needs::HostCommands => {
                gettext("which keeps it from asking the system’s own tools.")
            }
        };
        Some(format!(
            "{} {name}, {why} {}",
            gettext("Steward runs in"),
            gettext("The Steward package from Ubuntu’s apt repository can show this.")
        ))
    }

    /// Ubuntu's accent colour (Settings › Appearance), if the desktop has one.
    pub fn system_accent(&self) -> Option<&'static str> {
        let i = self.interface.as_ref()?;
        let schema = i.settings_schema()?;
        let read = |key: &str| schema.has_key(key).then(|| i.string(key).to_string());
        steward_core::desktop::system_accent(
            read("accent-color").as_deref(),
            read("gtk-theme").as_deref(),
        )
    }

    /// Brings the main window forward (shown again if it was hidden).
    pub fn present(&mut self) {
        self.emit(Event::Focus("raise"));
    }

    /// The window's close button: with the tray icon on, Steward keeps running
    /// in the background for the Shelf and tray (P8-T03); otherwise it quits.
    pub fn close_window(&mut self) {
        if settings::get().boolean("tray-icon") {
            self.emit(Event::Focus("hide"));
        } else {
            self.quit();
        }
    }

    pub fn set_width(&mut self, width: f64) {
        if (self.width - width).abs() > 1.0 {
            self.width = width;
            self.mark(dirty::ROWS);
        }
    }

    pub fn set_window(&mut self, width: i32, height: i32, active: bool) {
        self.window = (width, height, active);
    }

    /// The system's reduced-motion setting. End-to-end runs keep motion off
    /// unless a test asks for it with STEWARD_MOTION=1 (DESIGN.md §3.5).
    fn motion_wanted(&self) -> bool {
        if cfg!(feature = "e2e") && std::env::var("STEWARD_MOTION").as_deref() != Ok("1") {
            return false;
        }
        self.interface
            .as_ref()
            .is_none_or(|s| s.boolean("enable-animations"))
    }

    pub fn set_flying(&mut self, count: i32) {
        self.flying = count.max(0);
    }

    pub fn set_dark(&mut self, dark: bool) {
        self.dark = dark;
    }

    pub fn card(&mut self, name: &str, thumbnail: bool, count: &str, delta: i32) {
        let slot = self
            .cards
            .entry(name.to_string())
            .or_insert((0, false, String::new()));
        slot.0 += delta;
        if delta >= 0 {
            slot.1 = thumbnail;
            slot.2 = count.to_string();
        }
        if slot.0 <= 0 {
            self.cards.remove(name);
        }
    }

    pub fn row_bound(&mut self, delta: i32) {
        self.bound_rows += delta;
    }

    /// A card for `uri` came on screen (or left it): start or cancel its
    /// thumbnail, and count a folder's items. Cards are counted per file:
    /// when a grid rebuilds, Qt may destroy the old card after creating the
    /// new one, and that late "gone" must not cancel the new card's work.
    pub fn item_visible(&mut self, uri: &str, visible: bool) {
        let shown = {
            let count = self.on_screen.entry(uri.to_string()).or_insert(0);
            *count += if visible { 1 } else { -1 };
            *count
        };
        if shown <= 0 {
            self.on_screen.remove(uri);
        }
        let Some(entry) = self.visible.iter().find(|e| e.data().uri() == uri).cloned() else {
            return;
        };
        if entry.is_dir() {
            if visible {
                self.folder_seen(uri);
                self.count_items(&entry);
            }
            return;
        }
        if shown <= 0 {
            self.thumbnailer.cancel(&entry);
            return;
        }
        if !visible {
            return;
        }
        let uri = uri.to_string();
        if self.thumbs.contains_key(&uri) {
            return;
        }
        if self.thumb_watch.insert(uri.clone()) {
            entry.connect_notify_local(Some("thumbnail-path"), move |entry, _| {
                let path = entry.thumbnail_path();
                let uri = uri.clone();
                later(move |c| {
                    if let Some(path) = path {
                        c.thumbs.insert(uri, path);
                        c.mark(dirty::THUMBS | dirty::PREVIEW);
                    }
                });
            });
        }
        self.thumbnailer.request(&entry);
    }

    fn count_items(&mut self, entry: &FileEntry) {
        let uri = entry.data().uri();
        if self.counts.contains_key(&uri) || !self.counting.insert(uri.clone()) {
            return;
        }
        let file = entry.data().file.clone();
        glib::spawn_future_local(async move {
            let count = list_names(&file).await.len() as u32;
            later(move |c| {
                c.counting.remove(&uri);
                c.counts.insert(uri, count);
                c.mark(dirty::COUNTS);
            });
        });
    }

    // --- sidebar -------------------------------------------------------------------------------

    pub fn reload_sidebar(&mut self) {
        let (entries, hidden) = sidebar::load();
        self.sidebar = entries;
        self.hidden = hidden;
        // Preferences lists the Favorites with a switch each.
        self.mark(dirty::SIDEBAR | dirty::PREFS);
    }

    /// Hides a Favorites entry and says where to bring it back.
    pub fn sidebar_hide(&mut self, key: &str) {
        let label = self
            .sidebar
            .iter()
            .find(|e| e.key == key)
            .map(|e| e.label.clone());
        sidebar::hide(key);
        self.reload_sidebar();
        if let Some(label) = label {
            self.emit(Event::ToastWithUndo(
                gettext("{name} hidden. Bring it back in Preferences › General.")
                    .replace("{name}", &label),
                "win.sidebar-show".into(),
                key.into(),
            ));
        }
    }

    pub fn sidebar_show(&mut self, key: &str) {
        sidebar::show(key);
        self.reload_sidebar();
    }

    pub fn sidebar_move(&mut self, key: &str, delta: i32) {
        sidebar::move_by(&self.sidebar, key, delta);
        self.reload_sidebar();
    }

    pub fn sidebar_reset(&mut self) {
        sidebar::reset();
        self.reload_sidebar();
    }

    pub fn sidebar_eject(&mut self, key: &str) {
        if let Some(entry) = self.sidebar.iter().find(|e| e.key == key) {
            sidebar::eject(&entry.file);
        }
    }

    // --- JSON for QML ---------------------------------------------------------------------------

    fn folder_title(&self) -> String {
        if self.in_trash() {
            return gettext("Trash");
        }
        match self.current().and_then(|f| f.path()) {
            Some(path) if path == glib::home_dir() => gettext("Home"),
            Some(path) if path == std::path::Path::new("/") => gettext("Computer"),
            _ => self
                .current()
                .and_then(|f| f.basename())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        }
    }

    fn state_page(&self) -> (&'static str, String, String, String) {
        let state = self.state.clone();
        let status =
            |icon: &str, title: String, text: String| ("status", icon.to_string(), title, text);
        match state {
            LoadState::Loading => ("loading", String::new(), String::new(), String::new()),
            LoadState::PermissionDenied => status(
                "lock",
                gettext("You Can't Open This Folder"),
                gettext("You don't have permission to see what's inside."),
            ),
            LoadState::NotFound => status(
                "folder",
                gettext("This Folder Is Gone"),
                gettext("It was moved or deleted, or its drive is no longer connected."),
            ),
            LoadState::NotAFolder => status(
                "warning",
                gettext("This Is Not a Folder"),
                gettext("Open it from the folder that contains it."),
            ),
            LoadState::Failed(message) => {
                status("warning", gettext("Couldn't Open This Folder"), message)
            }
            LoadState::Ready if self.visible.is_empty() => {
                if !self.name_filter.is_empty() {
                    status(
                        "search",
                        gettext("No Matches"),
                        gettext("No names in this folder contain what you typed."),
                    )
                } else if let Some(kind) = self.kind_filter {
                    status(
                        "folder",
                        gettext("Nothing of This Kind"),
                        format!(
                            "{} {}",
                            gettext("No items in this folder are"),
                            format::kind(kind)
                        ),
                    )
                } else {
                    status(
                        "folder",
                        gettext("Folder Is Empty"),
                        gettext("Nothing is in this folder yet."),
                    )
                }
            }
            LoadState::Ready => (self.view.id(), String::new(), String::new(), String::new()),
        }
    }

    pub fn folder_json(&self) -> Value {
        let totals = self.kind_totals();
        let folders: u32 = totals
            .iter()
            .filter(|(k, _, _)| *k == Kind::Folder)
            .map(|(_, n, _)| n)
            .sum();
        let total: u32 = totals.iter().map(|(_, n, _)| n).sum();
        let files = total - folders;
        let bytes: u64 = totals
            .iter()
            .filter(|(k, _, _)| *k != Kind::Folder)
            .map(|(_, _, b)| b)
            .sum();
        let ready = self.state == LoadState::Ready;
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
        let selected = self.selection.len();
        let mut status = format::items(self.visible.len() as u32);
        if selected > 0 {
            status = format!("{status}, {selected} {}", gettext("selected"));
        }
        if let Some(free) = self.free {
            status = format!("{status} · {} {}", format::size(free), gettext("available"));
        }
        let mut chips = Vec::new();
        if total > 0 {
            chips.push(
                json!({ "id": "all", "label": gettext("All"), "count": total,
                               "active": self.kind_filter.is_none() }),
            );
            for (kind, count, _) in &totals {
                chips.push(
                    json!({ "id": kind.id(), "label": format::kind(*kind), "count": count,
                                   "active": self.kind_filter == Some(*kind) }),
                );
            }
        }
        let (page, status_icon, status_title, status_text) = self.state_page();
        json!({
            "title": self.folder_title(),
            "path": self.current().and_then(|f| f.path()).map(|p| p.display().to_string()),
            "uri": self.current().map(|f| f.uri().to_string()),
            "state": self.state.id(),
            "page": page,
            "statusIcon": status_icon,
            "statusTitle": status_title,
            "statusText": status_text,
            "summary": summary,
            "status": if ready { status } else { String::new() },
            "chips": chips,
            "viewMode": self.view.id(),
            "rowSize": self.row_size.id(),
            "groupBy": self.group.id(),
            "sort": self.sort.id(),
            "descending": self.descending,
            "filterKind": self.kind_filter.map(|k| k.id()).unwrap_or("all"),
            "nameFilter": self.name_filter,
            // The item the keyboard is on; the views keep it on screen.
            "anchor": self.anchor.map(|a| a as i64).unwrap_or(-1),
            // The one selected folder, for "Open in Terminal" on an item.
            "selectedFolder": match self.selected().as_slice() {
                [one] if one.is_dir() => one.data().file.path().map(|p| p.display().to_string()),
                _ => None,
            },
            "metrics": { "folder": self.row_size.folder_width(), "file": self.row_size.file_width(), "gap": GAP },
        })
    }

    fn columns(&self, card_width: i32) -> usize {
        ((self.width + f64::from(GAP)) / f64::from(card_width + GAP))
            .floor()
            .max(1.0) as usize
    }

    /// The name to show: in the Trash, the item's original name.
    fn shown_name(&self, entry: &FileEntry) -> String {
        let data = entry.data();
        match data.file.path() {
            Some(path) if self.in_trash() => self.trash_name(&path),
            _ => data.display_name.to_string(),
        }
    }

    fn item_json(&self, position: usize, entry: &FileEntry) -> Value {
        let data = entry.data();
        let look = match data.file.path() {
            Some(path) if data.is_dir => self.folder_look(&path, &data.uri(), &data.name),
            _ => (folder_symbol(data).to_string(), "folders".to_string()),
        };
        let meta = match data.file.path() {
            Some(path) if self.in_trash() => format!(
                "{} {}",
                gettext("From"),
                self.trash_origin(&path).unwrap_or_default()
            ),
            _ if data.is_dir => gettext("Folder"),
            _ => format!(
                "{} · {}",
                format::type_description(&data.content_type),
                format::size(data.size)
            ),
        };
        json!({
            "pos": position,
            "name": self.shown_name(entry),
            "uri": data.uri(),
            "dir": data.is_dir,
            "kind": data.kind.id(),
            "badge": tint::badge(&data.name, data.kind),
            "meta": meta,
            "symbol": look.0,
            "color": look.1,
        })
    }

    /// Grid rows: section headers and runs of cards (ADR 0008, §3.3).
    pub fn rows_json(&mut self) -> Value {
        let folder_columns = self.columns(self.row_size.folder_width());
        let file_columns = self.columns(self.row_size.file_width());
        let mut rows: Vec<Value> = Vec::new();
        let mut layout: Vec<Vec<usize>> = Vec::new();
        let section = |rows: &mut Vec<Value>,
                       layout: &mut Vec<Vec<usize>>,
                       title: String,
                       positions: Vec<usize>,
                       columns: usize,
                       me: &Self| {
            if positions.is_empty() {
                return;
            }
            rows.push(json!({ "t": "h", "title": title, "count": positions.len() }));
            for chunk in positions.chunks(columns) {
                rows.push(json!({ "t": "c", "items": chunk.iter().map(|&p| me.item_json(p, &me.visible[p])).collect::<Vec<_>>() }));
                layout.push(chunk.to_vec());
            }
        };
        let folders: Vec<usize> = (0..self.visible.len())
            .filter(|&p| self.visible[p].is_dir())
            .collect();
        let files: Vec<usize> = (0..self.visible.len())
            .filter(|&p| !self.visible[p].is_dir())
            .collect();
        match (self.group, self.kind_filter) {
            (GroupBy::Kind, None) => {
                section(
                    &mut rows,
                    &mut layout,
                    gettext("Folders"),
                    folders,
                    folder_columns,
                    self,
                );
                let mut kinds: Vec<(Kind, u32, u64)> = Vec::new();
                for entry in self.visible.iter().filter(|e| !e.is_dir()) {
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
                    rows.push(
                        json!({ "t": "h", "title": gettext("By kind"), "count": kinds.len() }),
                    );
                    for chunk in kinds.chunks(folder_columns) {
                        rows.push(
                            json!({ "t": "k", "kinds": chunk.iter().map(|(k, n, b)| json!({
                            "kind": k.id(), "label": format::kind(*k), "count": n,
                            "countText": format::files(*n), "sizeText": format::size(*b),
                        })).collect::<Vec<_>>() }),
                        );
                    }
                }
            }
            (GroupBy::Created, _) => {
                let now = app_data::now();
                let mut current: Option<(String, String, Vec<usize>)> = None;
                for (position, entry) in self.visible.iter().enumerate() {
                    let Some(s) = sort::section_of(entry.data(), GroupBy::Created, now) else {
                        continue;
                    };
                    match &mut current {
                        Some((id, _, positions)) if *id == s.id => positions.push(position),
                        _ => {
                            if let Some((_, title, positions)) = current.take() {
                                section(
                                    &mut rows,
                                    &mut layout,
                                    title,
                                    positions,
                                    file_columns,
                                    self,
                                );
                            }
                            current = Some((s.id, gettext(s.label), vec![position]));
                        }
                    }
                }
                if let Some((_, title, positions)) = current {
                    section(&mut rows, &mut layout, title, positions, file_columns, self);
                }
            }
            (_, Some(kind)) => {
                let columns = if kind == Kind::Folder {
                    folder_columns
                } else {
                    file_columns
                };
                section(
                    &mut rows,
                    &mut layout,
                    format::kind(kind),
                    (0..self.visible.len()).collect(),
                    columns,
                    self,
                );
            }
            (_, None) => {
                section(
                    &mut rows,
                    &mut layout,
                    gettext("Folders"),
                    folders,
                    folder_columns,
                    self,
                );
                section(
                    &mut rows,
                    &mut layout,
                    gettext("Files"),
                    files,
                    file_columns,
                    self,
                );
            }
        }
        self.rows = layout;
        Value::Array(rows)
    }

    #[cfg(feature = "e2e")]
    pub fn grid_headers(&self, rows: &Value) -> Vec<String> {
        rows.as_array()
            .map(|rows| {
                rows.iter()
                    .filter(|r| r["t"] == "h")
                    .map(|r| format!("{} {}", r["title"].as_str().unwrap_or(""), r["count"]))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// List rows, with a section label when grouping.
    pub fn items_json(&self) -> Value {
        // Only the list view shows these rows: the grid has its own (P6-T14).
        if self.view != ViewMode::List {
            return Value::Array(Vec::new());
        }
        let now = app_data::now();
        Value::Array(
            self.visible
                .iter()
                .enumerate()
                .map(|(position, entry)| {
                    let data = entry.data();
                    let section = sort::section_of(data, self.group, now).map(|s| gettext(s.label)).unwrap_or_default();
                    json!({
                        "pos": position,
                        "name": self.shown_name(entry),
                        "uri": data.uri(),
                        "dir": data.is_dir,
                        "kind": data.kind.id(),
                        "kindText": if data.is_dir { gettext("Folder") } else { format::kind(data.kind) },
                        "sizeText": if data.is_dir { "—".to_string() } else { format::size(data.size) },
                        "modified": format::date(data.modified),
                        "created": format::date(data.created),
                        "section": section,
                    })
                })
                .collect(),
        )
    }

    pub fn selection_json(&self) -> Value {
        let mut map = serde_json::Map::new();
        for (position, entry) in self.visible.iter().enumerate() {
            if self.selection.contains(&entry.data().uri()) {
                map.insert(position.to_string(), Value::Bool(true));
            }
        }
        Value::Object(map)
    }

    pub fn thumbs_json(&self) -> Value {
        json!(self.thumbs)
    }

    pub fn sidebar_json(&self) -> Value {
        let selected = self.current().and_then(|f| {
            self.sidebar
                .iter()
                .find(|e| e.file.equal(&f))
                .map(|e| e.key.clone())
        });
        let (free, usage) = match self.home_space {
            Some((free, size)) if size > 0 => (
                format!("{} {}", format::size(free), gettext("free")),
                1.0 - free as f64 / size as f64,
            ),
            _ => (String::new(), 0.0),
        };
        json!({
            "entries": self.sidebar.iter().map(|e| json!({
                "key": e.key, "label": e.label, "section": e.section, "icon": e.icon, "eject": e.can_eject,
                "uri": e.file.uri().to_string(),
                "path": e.file.path().map(|p| p.display().to_string()).unwrap_or_else(|| e.file.uri().to_string()),
            })).collect::<Vec<_>>(),
            "workspaces": self.workspaces_json(),
            "bookmarks": self.bookmarks_sidebar_json(),
            "tools": [
                { "id": "clean", "label": gettext("Free Up Space"), "icon": "gauge",
                  "selected": self.tools.page == "clean" },
                { "id": "installed", "label": gettext("Installed"), "icon": "package",
                  "selected": self.tools.page == "installed", "count": self.installed_count() },
                { "id": "agents", "label": gettext("AI Agents"), "icon": "agent",
                  "selected": self.tools.page == "agents" },
            ],
            "page": self.tools.page,
            "hidden": self.hidden,
            "selected": selected,
            "expanded": settings::get().boolean("sidebar-visible"),
            "free": free,
            "usage": usage,
        })
    }

    pub fn preview_json(&self) -> Value {
        let visible = settings::get().boolean("preview-visible");
        let selected = self.selected();
        if let [entry] = selected.as_slice() {
            let data = entry.data();
            let kind = if data.is_dir {
                gettext("Folder")
            } else {
                gio::content_type_get_description(&data.content_type).to_string()
            };
            let mut facts = vec![json!([gettext("Kind"), kind])];
            if !data.is_dir {
                facts.push(json!([gettext("Size"), format::size(data.size)]));
            }
            facts.push(json!([gettext("Modified"), format::date(data.modified)]));
            facts.push(json!([gettext("Created"), format::date(data.created)]));
            if let Some(parent) = data.file.parent().and_then(|p| p.path()) {
                facts.push(json!([gettext("Where"), paths::display_path(&parent)]));
            }
            let mut children = match &self.preview_children {
                Some((uri, names)) if *uri == data.uri() => names.clone(),
                _ => Vec::new(),
            };
            // The index knows the folder two levels deep (P3-T06).
            let path = data.file.path();
            let tree = path
                .as_deref()
                .filter(|_| data.is_dir)
                .and_then(|p| self.outline_json(p));
            let numbers = path
                .as_deref()
                .filter(|_| data.is_dir)
                .and_then(|p| self.folder_numbers(p));
            if let Some(tree) = &tree
                && children.is_empty()
            {
                children = tree
                    .as_array()
                    .map(|lines| {
                        lines
                            .iter()
                            .filter(|l| l["depth"] == 0)
                            .filter_map(|l| l["name"].as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
            }
            if let Some((_, bytes)) = numbers {
                facts.insert(1, json!([gettext("Size"), format::size(bytes)]));
            }
            return json!({
                "visible": visible, "mode": "item", "name": self.shown_name(entry), "uri": data.uri(),
                "dir": data.is_dir, "kind": data.kind.id(), "badge": tint::badge(&data.name, data.kind),
                "thumb": self.thumbs.get(&data.uri()), "facts": facts,
                "children": children.iter().take(12).collect::<Vec<_>>(),
                "childCount": format::items(numbers.map(|n| n.0 as u32).unwrap_or(children.len() as u32)),
                "tree": tree,
                "more": children.len().saturating_sub(12),
            });
        }
        if self.state != LoadState::Ready {
            return json!({ "visible": visible, "mode": "empty" });
        }
        let totals = self.kind_totals();
        let folders: u32 = totals
            .iter()
            .filter(|(k, _, _)| *k == Kind::Folder)
            .map(|(_, n, _)| n)
            .sum();
        let mut kinds: Vec<(Kind, u32, u64)> = totals
            .iter()
            .copied()
            .filter(|(k, _, _)| *k != Kind::Folder)
            .collect();
        let files: u32 = kinds.iter().map(|(_, n, _)| n).sum();
        let bytes: u64 = kinds.iter().map(|(_, _, b)| b).sum();
        kinds.sort_by_key(|k| std::cmp::Reverse(k.2));
        let largest = kinds.first().map(|k| k.2).unwrap_or(1).max(1);
        json!({
            "visible": visible,
            "mode": "overview",
            "suggestions": self.suggestions_json(),
            "title": self.folder_title(),
            "total": format::size(bytes),
            "counts": format!("{} · {}", format::folders(folders), format::files(files)),
            "kinds": kinds.iter().take(5).map(|(k, _, b)| json!({
                "kind": k.id(), "label": format::kind(*k), "size": format::size(*b), "frac": *b as f64 / largest as f64,
            })).collect::<Vec<_>>(),
            "more": kinds.len().saturating_sub(5),
        })
    }

    pub fn path_json(&self) -> Value {
        let path = self
            .current()
            .and_then(|f| f.path())
            .unwrap_or_else(|| PathBuf::from("/"));
        // The Trash is a place of its own, not a folder under .local.
        let segments = if self.in_trash() {
            vec![(gettext("Trash"), path.clone())]
        } else {
            paths::segments(&path, &gettext("Computer"))
        };
        json!({
            "segments": segments.into_iter()
                .map(|(label, p)| json!({ "label": label, "path": p.display().to_string() }))
                .collect::<Vec<_>>(),
            "editing": self.path_editing,
            "git": self.git_json(),
            "text": self.path_text.clone().unwrap_or_else(|| paths::display_path(&path)),
            "matches": self.path_matches.iter().enumerate().map(|(i, p)| json!({
                "index": i,
                "name": p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                "where": p.parent().map(paths::display_path).unwrap_or_default(),
                "chosen": self.path_match == Some(i),
            })).collect::<Vec<_>>(),
        })
    }

    pub fn nav_json(&self) -> Value {
        json!({
            "back": !self.back.is_empty() || !self.tools.page.is_empty(),
            "forward": !self.forward.is_empty(),
            "up": self.current().and_then(|f| f.parent()).is_some(),
        })
    }

    pub fn prefs_json(&mut self) -> Value {
        let s = settings::get();
        json!({
            "colorScheme": s.string("color-scheme").to_string(),
            "accent": s.string("accent").to_string(),
            "rowSize": s.string("row-size").to_string(),
            "defaultView": s.string("default-view").to_string(),
            "showHidden": s.boolean("show-hidden"),
            "preview": s.boolean("preview-visible"),
            "sidebar": s.boolean("sidebar-visible"),
            "sidebarItems": sidebar::favorites().into_iter().map(|(key, label, icon, hidden)| json!({
                "key": key, "label": label, "icon": icon, "shown": !hidden,
            })).collect::<Vec<_>>(),
            "width": s.int("window-width"),
            "height": s.int("window-height"),
            "maximized": s.boolean("window-maximized"),
            "motion": self.motion,
            // Search and index (P3-T12).
            "downloadsFolder": crate::paths::display_path(&downloads_folder()),
            "indexRoots": s.strv("index-roots").iter().map(|r| r.to_string()).collect::<Vec<_>>(),
            "indexExclude": s.strv("index-exclude").iter().map(|r| r.to_string()).collect::<Vec<_>>(),
            "indexPaused": s.boolean("index-paused"),
            "indexPauseOnBattery": s.boolean("index-pause-on-battery"),
            "indexRunning": self.index.running,
            "home": paths::display_path(&glib::home_dir()),
            "defaultFileManager": s.boolean("default-file-manager"),
            "bookmarkFetch": s.boolean("bookmark-fetch"),
            "shelfShortcutEnabled": s.boolean("shelf-shortcut-enabled"),
            "shelfShortcut": s.string("shelf-shortcut").to_string(),
            "trayIcon": s.boolean("tray-icon"),
            "providerWorkspaces": s.boolean("search-provider-workspaces"),
            "providerCollections": s.boolean("search-provider-collections"),
            "providerLinks": s.boolean("search-provider-links"),
            "openAtLogin": s.boolean("open-at-login"),
            "openIn": self.openin_json(),
            // Ubuntu's accent colour (Settings › Appearance): the colour when
            // the style follows the system.
            "systemAccentColor": self.system_accent(),
            // GNOME's dark mode, for Qt versions that cannot report it (24.04).
            "systemDark": self.interface.as_ref().is_some_and(|i| {
                i.settings_schema()
                    .is_some_and(|schema| schema.has_key("color-scheme"))
                    && i.string("color-scheme") == "prefer-dark"
            }),
            // Test hook: force the Qt 6.4 fallbacks on any Qt.
            "legacyFx": std::env::var("STEWARD_LEGACY_FX").is_ok_and(|v| v == "1"),
        })
    }

    // --- test state (e2e builds) --------------------------------------------------------------

    #[cfg(feature = "e2e")]
    pub fn test_state(&mut self) -> Value {
        let folder = self.folder_json();
        let rows = self.rows_json();
        let preview = self.preview_json();
        let path = self.path_json();
        let sidebar = self.sidebar_json();
        let names = |entries: Vec<&FileEntry>| -> Vec<String> {
            entries.iter().map(|e| e.name().to_string()).collect()
        };
        let sections: Vec<Value> = {
            let now = app_data::now();
            let mut out: Vec<(String, String, u32)> = Vec::new();
            for entry in &self.visible {
                if let Some(s) = sort::section_of(entry.data(), self.group, now) {
                    match out.last_mut() {
                        Some(last) if last.0 == s.id => last.2 += 1,
                        _ => out.push((s.id, gettext(s.label), 1)),
                    }
                }
            }
            out.into_iter()
                .map(|(id, label, count)| json!({ "id": id, "label": label, "count": count }))
                .collect()
        };
        let cards: Vec<Value> = self
            .cards
            .iter()
            .map(|(name, (_, thumbnail, count))| json!({ "name": name, "thumbnail": thumbnail, "count": count }))
            .collect();
        let expanded = sidebar["expanded"].as_bool().unwrap_or(true);
        json!({
            "folder": {
                "uri": folder["uri"], "path": folder["path"], "title": folder["title"],
                "summary": folder["summary"], "status": folder["status"], "state": folder["state"],
                "page": folder["page"], "status_title": folder["statusTitle"],
                "view_mode": folder["viewMode"], "row_size": folder["rowSize"], "sort": folder["sort"],
                "descending": folder["descending"], "group_by": folder["groupBy"],
                "show_hidden": self.show_hidden, "filter_kind": folder["filterKind"],
                "name_filter": folder["nameFilter"],
                "items": names(self.visible.iter().collect()),
                "selection": names(self.selected()),
                "sections": sections,
                "chips": folder["chips"],
                "grid_headers": self.grid_headers(&rows),
                "bound_cards": self.cards.len(),
                "bound_rows": self.bound_rows,
                "cards": cards,
                "last_load_ms": self.last_load_ms,
            },
            "navigation": { "can_go_back": !self.back.is_empty(), "can_go_forward": !self.forward.is_empty() },
            "path_bar": {
                "matches": path["matches"],
                "editing": self.path_editing,
                "text": path["text"],
                "segments": path["segments"].as_array().map(|s| s.iter().map(|x| x["label"].clone()).collect::<Vec<_>>()),
                "git": path["git"],
            },
            "sidebar": {
                "entries": sidebar["entries"], "hidden": sidebar["hidden"], "selected": sidebar["selected"],
                "expanded": expanded, "visible": expanded, "rail_visible": !expanded,
                "rail": self.sidebar.iter().map(|e| e.label.clone()).collect::<Vec<_>>(),
            },
            "preview": {
                "visible": preview["visible"], "name": preview["name"],
                "facts": preview["facts"].as_array().cloned().unwrap_or_default(),
                "children": preview["children"].as_array().cloned().unwrap_or_default(),
            },
            "last_toast": self.last_toast,
            "last_report": self.last_report,
            "ops": self.ops_test_json(),
            "index": self.index_test_json(),
            "search": self.search_test_json(),
            "peek": self.peek_test_json(),
            "tidy": self.tidy_test_json(),
            "downloads": self.downloads_test_json(),
            "pane": self.pane_test_json(),
            "workspace": self.workspace_test_json(),
            "folder_styles": self.styles_test_json(),
            "tools": { "page": self.tools.page },
            "clean": self.clean_test_json(),
            "installed": self.installed_test_json(),
            "about": self.about_test_json(),
            "welcome": self.welcome_test_json(),
            "openin": self.openin_test_json(),
            "agents": self.agents_test_json(),
            "markdown": self.markdown_test_json(),
            "bookmarks": self.bookmarks_test_json(),
            "shelf": self.shelf_test_json(),
            "tray_item": crate::tray::item_name(),
            // Folder numbers as the cards show them, by name.
            "folder_stats": self.visible.iter().filter(|e| e.is_dir()).filter_map(|e| {
                let (items, bytes) = self.folder_numbers(&e.data().file.path()?)?;
                Some((e.name().to_string(), json!({ "items": items, "bytes": bytes })))
            }).collect::<serde_json::Map<_, _>>(),
            "dark": self.dark,
            "sandbox": self.sandbox.map(|s| s.name()),
            "appearance": {
                "style": settings::get().string("color-scheme").to_string(),
                "accent": settings::get().string("accent").to_string(),
                "system_accent": self.system_accent(),
            },
            "motion": self.motion,
            "flying": self.flying,
            "dialog": self.dialog,
            "window": { "width": self.window.0, "height": self.window.1, "is_active": self.window.2 },
        })
    }
}

/// A file card's symbol: its kind's (folders get theirs from style.rs).
fn folder_symbol(data: &steward_core::EntryData) -> &'static str {
    tint_symbol(data.kind)
}

fn tint_symbol(kind: Kind) -> &'static str {
    match kind {
        Kind::Folder => "folder",
        Kind::Images => "pictures",
        Kind::Videos => "videos",
        Kind::Audio => "music",
        Kind::Pdfs => "pdf",
        Kind::Documents => "document",
        Kind::Archives => "box",
        Kind::Installers => "package",
        Kind::Code => "code",
        Kind::Other => "more",
    }
}

/// GNOME's interface settings, when the schema is installed.
/// Flatpak or Snap (P9-T01). Test builds can simulate one (E9-01).
fn detect_sandbox() -> Option<steward_core::sandbox::Sandbox> {
    let simulate = cfg!(feature = "e2e") && std::env::var_os("STEWARD_SIMULATE_SANDBOX").is_some();
    steward_core::sandbox::detect(
        std::path::Path::new("/"),
        std::env::var("SNAP").ok().as_deref(),
        simulate,
    )
}

fn interface_settings() -> Option<gio::Settings> {
    let schema =
        gio::SettingsSchemaSource::default()?.lookup("org.gnome.desktop.interface", true)?;
    schema
        .has_key("enable-animations")
        .then(|| gio::Settings::new("org.gnome.desktop.interface"))
}

/// Names of the non-hidden items in a folder, sorted.
async fn list_names(folder: &gio::File) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(enumerator) = folder
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
            names.extend(
                infos
                    .iter()
                    .filter(|i| !i.is_hidden() && !i.name().to_string_lossy().starts_with('.'))
                    .map(|i| i.name().to_string_lossy().into_owned()),
            );
        }
    }
    names.sort_by(|a, b| steward_core::natural::compare(a, b));
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_sizes_round_trip_and_step() {
        for size in [RowSize::Small, RowSize::Medium, RowSize::Large] {
            assert_eq!(RowSize::from_id(size.id()), Some(size));
        }
        assert_eq!(RowSize::Small.bigger(), RowSize::Medium);
        assert_eq!(RowSize::Large.bigger(), RowSize::Large);
        assert_eq!(RowSize::Small.smaller(), RowSize::Small);
        assert!(RowSize::Large.file_width() > RowSize::Small.file_width());
    }

    #[test]
    fn view_modes_round_trip() {
        assert_eq!(ViewMode::from_id("list"), Some(ViewMode::List));
        assert_eq!(ViewMode::from_id("nope"), None);
    }
}
