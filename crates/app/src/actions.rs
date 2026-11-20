//! Every user command is a named GIO action (AGENTS.md rule 11). Two groups,
//! `app` and `win`, exported on D-Bus as org.gtk.Actions (see dbus.rs) and
//! activated by QML shortcuts and menus through the presenter, so keys, menus,
//! the command-line tool and the tests all take the same path.

use std::cell::RefCell;
use std::collections::HashMap;

use gio::prelude::*;
use serde_json::{Value, json};

use cairn_core::Kind;
use cairn_core::sort::{GroupBy, SortKey};

use crate::controller::{self, RowSize, ViewMode};
use crate::settings;

/// Keyboard shortcuts (DESIGN.md Appendix A), in Qt key-sequence syntax.
/// QML creates one Shortcut per entry; the shortcuts dialog lists them.
pub const ACCELS: &[(&str, &str, &[&str], &str)] = &[
    // (action, parameter, keys, description)
    ("app.quit", "", &["Ctrl+Q"], "Quit"),
    ("app.preferences", "", &["Ctrl+,"], "Preferences"),
    (
        "win.show-help-overlay",
        "",
        &["Ctrl+?", "Ctrl+Shift+/"],
        "Keyboard shortcuts",
    ),
    ("win.back", "", &["Alt+Left", "Back"], "Back"),
    ("win.forward", "", &["Alt+Right", "Forward"], "Forward"),
    ("win.up", "", &["Alt+Up"], "Up to the parent folder"),
    ("win.home", "", &["Alt+Home"], "Home folder"),
    ("win.reload", "", &["Ctrl+R"], "Reload"),
    (
        "win.location",
        "",
        &["Ctrl+L"],
        "Type a path or a folder name",
    ),
    ("win.filter", "", &["Ctrl+F"], "Filter this folder"),
    (
        "win.open-selected",
        "",
        &["Ctrl+O"],
        "Open the selected item",
    ),
    ("win.view-mode", "grid", &["Ctrl+1"], "Grid"),
    ("win.view-mode", "list", &["Ctrl+2"], "List"),
    ("win.zoom-in", "", &["Ctrl++", "Ctrl+="], "Larger"),
    ("win.zoom-out", "", &["Ctrl+-"], "Smaller"),
    ("win.zoom-reset", "", &["Ctrl+0"], "Normal size"),
    ("win.show-hidden", "", &["Ctrl+H"], "Show hidden files"),
    (
        "win.toggle-sidebar",
        "",
        &["F9"],
        "Collapse or expand the sidebar",
    ),
    (
        "win.toggle-preview",
        "",
        &["Ctrl+Shift+P"],
        "Show the preview pane",
    ),
    ("win.new-folder", "", &["Ctrl+Shift+N"], "New folder"),
    ("win.rename", "", &["F2"], "Rename"),
    ("win.trash", "", &["Delete"], "Move to the Trash"),
    ("win.copy", "", &["Ctrl+C"], "Copy"),
    ("win.cut", "", &["Ctrl+X"], "Cut"),
    ("win.paste", "", &["Ctrl+V"], "Paste"),
    ("win.undo", "", &["Ctrl+Z"], "Undo"),
    ("win.redo", "", &["Ctrl+Shift+Z"], "Redo"),
    (
        "win.search",
        "",
        &["Ctrl+K"],
        "Find anything: folders and files",
    ),
    ("win.peek", "", &["Space"], "Peek at a file"),
    ("win.tidy", "", &["Ctrl+Shift+O"], "Tidy this folder"),
    ("win.dual-pane", "", &["F3"], "Split into two panes"),
    ("win.switch-pane", "", &["Tab"], "Switch pane"),
    ("win.copy-across", "", &["F5"], "Copy to the other pane"),
    ("win.move-across", "", &["F6"], "Move to the other pane"),
    ("win.swap-panes", "", &["Ctrl+U"], "Swap the two panes"),
];

/// Shortcuts that text fields need for themselves: QML turns these off while
/// one has the focus.
const TEXT_KEYS: &[&str] = &[
    "win.trash",
    "win.copy",
    "win.cut",
    "win.paste",
    "win.undo",
    "win.redo",
    "win.peek",
    "win.switch-pane",
];

thread_local! {
    static GROUPS: RefCell<Option<(gio::SimpleActionGroup, gio::SimpleActionGroup)>> = const { RefCell::new(None) };
    static ACTIONS: RefCell<HashMap<String, gio::SimpleAction>> = RefCell::new(HashMap::new());
}

/// The app and win groups (created on first use).
pub fn groups() -> (gio::SimpleActionGroup, gio::SimpleActionGroup) {
    GROUPS.with(|cell| cell.borrow_mut().get_or_insert_with(create).clone())
}

pub fn accels_json() -> Value {
    Value::Array(
        ACCELS
            .iter()
            .map(|(action, param, keys, description)| {
                json!({ "action": action, "param": param, "keys": keys, "description": description,
                        "typing": TEXT_KEYS.contains(action) })
            })
            .collect(),
    )
}

/// Activates `scope.name` (e.g. "win.back") with an optional string parameter.
pub fn activate(detailed: &str, param: &str) {
    let (app, win) = groups();
    let Some((scope, name)) = detailed.split_once('.') else {
        return;
    };
    let group = if scope == "app" { app } else { win };
    let Some(action) = group.lookup_action(name) else {
        glib::g_warning!("cairn", "no action {detailed}");
        return;
    };
    let wants_string = action
        .parameter_type()
        .is_some_and(|t| t == *glib::VariantTy::STRING);
    let parameter = wants_string.then(|| param.to_variant());
    action.activate(parameter.as_ref());
}

pub fn set_state(name: &str, state: &glib::Variant) {
    ACTIONS.with(|map| {
        if let Some(action) = map.borrow().get(name) {
            action.set_state(state);
        }
    });
}

pub fn set_enabled(name: &str, enabled: bool) {
    ACTIONS.with(|map| {
        if let Some(action) = map.borrow().get(name) {
            action.set_enabled(enabled);
        }
    });
}

fn remember(action: &gio::SimpleAction) {
    ACTIONS.with(|map| {
        map.borrow_mut()
            .insert(action.name().to_string(), action.clone())
    });
}

fn simple(group: &gio::SimpleActionGroup, name: &str, run: fn()) {
    let action = gio::SimpleAction::new(name, None);
    action.connect_activate(move |_, _| run());
    group.add_action(&action);
    remember(&action);
}

fn with_string(group: &gio::SimpleActionGroup, name: &str, run: fn(&str)) {
    let action = gio::SimpleAction::new(name, Some(glib::VariantTy::STRING));
    action.connect_activate(move |_, param| {
        if let Some(value) = param.and_then(|p| p.get::<String>()) {
            run(&value);
        }
    });
    group.add_action(&action);
    remember(&action);
}

fn choice(group: &gio::SimpleActionGroup, name: &str, initial: &str, apply: fn(&str)) {
    let action =
        gio::SimpleAction::new_stateful(name, Some(glib::VariantTy::STRING), &initial.to_variant());
    action.connect_change_state(move |action, value| {
        if let Some(value) = value.and_then(|v| v.get::<String>()) {
            action.set_state(&value.to_variant());
            apply(&value);
        }
    });
    group.add_action(&action);
    remember(&action);
}

fn toggle(group: &gio::SimpleActionGroup, name: &str, initial: bool, apply: fn(bool)) {
    let action = gio::SimpleAction::new_stateful(name, None, &initial.to_variant());
    action.connect_change_state(move |action, value| {
        if let Some(value) = value.and_then(|v| v.get::<bool>()) {
            action.set_state(&value.to_variant());
            apply(value);
        }
    });
    group.add_action(&action);
    remember(&action);
}

fn set_row_size(change: fn(RowSize) -> RowSize) {
    let s = settings::get();
    let size = change(RowSize::from_id(&s.string("row-size")).unwrap_or_default());
    let _ = s.set_string("row-size", size.id());
}

fn create() -> (gio::SimpleActionGroup, gio::SimpleActionGroup) {
    let s = settings::get();
    let app = gio::SimpleActionGroup::new();
    simple(&app, "quit", || controller::with(|c| c.quit()));
    simple(&app, "about", || {
        controller::with(|c| c.show_dialog("about"))
    });
    simple(&app, "preferences", || {
        controller::with(|c| c.show_dialog("preferences"))
    });
    let scheme = s.create_action("color-scheme");
    app.add_action(&scheme);

    let win = gio::SimpleActionGroup::new();
    simple(&win, "back", || controller::with(|c| c.go_back()));
    simple(&win, "forward", || controller::with(|c| c.go_forward()));
    simple(&win, "up", || controller::with(|c| c.go_up()));
    simple(&win, "home", || {
        controller::with(|c| c.navigate(&gio::File::for_path(glib::home_dir()), true))
    });
    simple(&win, "reload", || controller::with(|c| c.reload()));
    simple(&win, "location", || {
        controller::with(|c| c.start_path_edit())
    });
    simple(&win, "filter", || controller::with(|c| c.focus_filter()));
    // File operations (Phase 2).
    simple(&win, "new-folder", || controller::with(|c| c.new_folder()));
    simple(&win, "rename", || controller::with(|c| c.start_rename()));
    simple(&win, "trash", || controller::with(|c| c.trash_selected()));
    simple(&win, "duplicate", || controller::with(|c| c.duplicate()));
    simple(&win, "copy", || {
        controller::with(|c| c.copy_selection(false))
    });
    simple(&win, "cut", || controller::with(|c| c.copy_selection(true)));
    simple(&win, "paste", || controller::with(|c| c.paste()));
    simple(&win, "undo", || controller::with(|c| c.undo()));
    simple(&win, "redo", || controller::with(|c| c.redo()));
    simple(&win, "restore", || {
        controller::with(|c| c.restore_selected())
    });
    simple(&win, "empty-trash", || {
        controller::with(|c| c.ask_empty_trash())
    });
    simple(&win, "empty-trash-confirm", || {
        controller::with(|c| c.confirm_empty_trash())
    });
    with_string(&win, "cancel-job", |id| {
        controller::with(|c| c.cancel_job(id))
    });
    // Search, Peek and the index (Phase 3).
    simple(&win, "search", || controller::with(|c| c.open_search()));
    simple(&win, "search-close", || {
        controller::with(|c| c.close_search())
    });
    with_string(&win, "search-move", |key| {
        controller::with(|c| c.search_move(key))
    });
    with_string(&win, "search-select", |i| {
        controller::with(|c| c.search_select(i.parse().unwrap_or(0)))
    });
    with_string(&win, "search-kind", |kind| {
        controller::with(|c| c.search_kind(kind))
    });
    simple(&win, "search-open", || {
        controller::with(|c| c.search_open())
    });
    simple(&win, "search-reveal", || {
        controller::with(|c| c.search_reveal())
    });
    simple(&win, "search-peek", || {
        controller::with(|c| c.search_peek())
    });
    simple(&win, "peek", || controller::with(|c| c.peek_selected()));
    with_string(&win, "peek-move", |d| controller::with(|c| c.peek_move(d)));
    simple(&win, "peek-close", || controller::with(|c| c.peek_close()));
    simple(&win, "peek-open", || controller::with(|c| c.peek_open()));
    with_string(&win, "path-match", |key| {
        controller::with(|c| match key.parse::<usize>() {
            Ok(i) => c.path_match_go(i),
            Err(_) => c.path_match_move(key),
        })
    });
    simple(&win, "start-indexer", || {
        controller::with(|c| c.start_indexer())
    });
    simple(&win, "tidy", || controller::with(|c| c.plan_tidy()));
    simple(&win, "dual-pane", || {
        controller::with(|c| c.toggle_dual_pane())
    });
    simple(&win, "switch-pane", || {
        controller::with(|c| c.switch_pane())
    });
    simple(&win, "copy-across", || {
        controller::with(|c| c.across(false))
    });
    simple(&win, "move-across", || controller::with(|c| c.across(true)));
    simple(&win, "swap-panes", || controller::with(|c| c.swap_panes()));
    with_string(&win, "pane-go", |path| {
        controller::with(|c| c.pane_go(path))
    });
    with_string(&win, "pane-key", |key| {
        controller::with(|c| c.pane_key(key))
    });
    with_string(&win, "pane-open", |pos| {
        controller::with(|c| c.pane_open(pos.parse().unwrap_or(0)))
    });
    with_string(&win, "pane-pick", |arg| {
        let (pos, mode) = arg.split_once(':').unwrap_or((arg, "0"));
        controller::with(|c| {
            c.pane_pick(pos.parse().unwrap_or(usize::MAX), mode.parse().unwrap_or(0))
        })
    });
    with_string(&win, "pane-activate", |side| {
        controller::with(|c| c.pane_activate(side == "right"))
    });
    simple(&win, "tidy-downloads", || {
        controller::with(|c| c.tidy_downloads())
    });
    with_string(&win, "suggest-trash", |id| {
        controller::with(|c| c.trash_suggestion(id))
    });
    with_string(&win, "set-downloads", |path| {
        controller::with(|c| c.set_downloads_folder(path))
    });
    simple(&win, "tidy-run", || controller::with(|c| c.run_tidy()));
    simple(&win, "tidy-cancel", || {
        controller::with(|c| c.cancel_tidy())
    });
    with_string(&win, "index-paused", |v| {
        controller::with(|c| c.set_index_paused(v == "true"))
    });
    simple(&win, "index-rebuild", || {
        controller::with(|c| c.rebuild_index())
    });
    with_string(&win, "index-exclude", |change| {
        controller::with(|c| c.edit_index_exclude(change))
    });
    simple(&win, "open-selected", || {
        controller::with(|c| c.open_selected())
    });
    simple(&win, "open-with", || controller::with(|c| c.open_with()));
    simple(&win, "select-all", || controller::with(|c| c.select_all()));
    simple(&win, "show-help-overlay", || {
        controller::with(|c| c.show_dialog("shortcuts"))
    });
    simple(&win, "zoom-in", || set_row_size(RowSize::bigger));
    simple(&win, "zoom-out", || set_row_size(RowSize::smaller));
    simple(&win, "zoom-reset", || set_row_size(|_| RowSize::Medium));
    simple(&win, "sidebar-reset", || {
        controller::with(|c| c.sidebar_reset())
    });
    with_string(&win, "go-to", |target| {
        controller::with(|c| c.go_to(target))
    });
    with_string(&win, "sidebar-hide", |key| {
        controller::with(|c| c.sidebar_hide(key))
    });
    with_string(&win, "sidebar-move-up", |key| {
        controller::with(|c| c.sidebar_move(key, -1))
    });
    with_string(&win, "sidebar-move-down", |key| {
        controller::with(|c| c.sidebar_move(key, 1))
    });
    with_string(&win, "sidebar-eject", |key| {
        controller::with(|c| c.sidebar_eject(key))
    });
    choice(&win, "view-mode", "grid", |id| {
        controller::with(|c| c.set_view(ViewMode::from_id(id).unwrap_or_default()))
    });
    choice(&win, "sort-by", "name", |id| {
        controller::with(|c| {
            let descending = c.descending;
            c.set_sort(SortKey::from_id(id).unwrap_or_default(), descending)
        })
    });
    choice(&win, "group-by", "none", |id| {
        controller::with(|c| c.set_group(GroupBy::from_id(id).unwrap_or_default()))
    });
    choice(&win, "filter-kind", "all", |id| {
        controller::with(|c| c.set_kind_filter(Kind::from_id(id)))
    });
    toggle(&win, "sort-descending", false, |descending| {
        controller::with(|c| {
            let key = c.sort;
            c.set_sort(key, descending)
        })
    });
    toggle(
        &win,
        "toggle-preview",
        s.boolean("preview-visible"),
        |visible| {
            let _ = settings::get().set_boolean("preview-visible", visible);
        },
    );
    toggle(
        &win,
        "toggle-sidebar",
        s.boolean("sidebar-visible"),
        |visible| {
            let _ = settings::get().set_boolean("sidebar-visible", visible);
        },
    );
    win.add_action(&s.create_action("show-hidden"));
    (app, win)
}
