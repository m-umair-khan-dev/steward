//! Every user command is a named GIO action (AGENTS.md rule 11). Two groups,
//! `app` and `win`, exported on D-Bus as org.gtk.Actions (see dbus.rs) and
//! activated by QML shortcuts and menus through the presenter, so keys, menus,
//! the command-line tool and the tests all take the same path.

use std::cell::RefCell;
use std::collections::HashMap;

use gio::prelude::*;
use serde_json::{Value, json};

use steward_core::Kind;
use steward_core::sort::{GroupBy, SortKey};

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
    ("win.tool-open", "clean", &["Ctrl+Shift+K"], "Free Up Space"),
    (
        "win.open-terminal",
        "",
        &["F4"],
        "Open this folder in the terminal",
    ),
    (
        "win.open-editor",
        "",
        &["Shift+F4"],
        "Open this folder in the editor",
    ),
    ("win.md-save", "", &["Ctrl+S"], "Save the Markdown file"),
    (
        "win.bookmark-add-ask",
        "",
        &["Ctrl+Shift+L"],
        "Add a link to Bookmarks",
    ),
    ("win.workspace-open", "#1", &["Alt+1"], "Open workspace 1"),
    ("win.workspace-open", "#2", &["Alt+2"], "Open workspace 2"),
    ("win.workspace-open", "#3", &["Alt+3"], "Open workspace 3"),
    ("win.workspace-open", "#4", &["Alt+4"], "Open workspace 4"),
    ("win.workspace-open", "#5", &["Alt+5"], "Open workspace 5"),
    ("win.workspace-open", "#6", &["Alt+6"], "Open workspace 6"),
    ("win.workspace-open", "#7", &["Alt+7"], "Open workspace 7"),
    ("win.workspace-open", "#8", &["Alt+8"], "Open workspace 8"),
    ("win.workspace-open", "#9", &["Alt+9"], "Open workspace 9"),
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

// --- customisable shortcuts (P7-T11) -------------------------------------------------------

/// An entry's id in the `shortcuts` setting: "win.tidy", or
/// "win.view-mode('list')" for an entry with a parameter.
pub fn accel_id(action: &str, param: &str) -> String {
    if param.is_empty() {
        action.to_string()
    } else {
        format!("{action}('{param}')")
    }
}

/// The user's changes: entry id → keys (empty: no shortcut).
fn overrides() -> HashMap<String, Vec<String>> {
    settings::get()
        .value("shortcuts")
        .get::<HashMap<String, Vec<String>>>()
        .unwrap_or_default()
}

fn set_overrides(map: &HashMap<String, Vec<String>>) {
    let _ = settings::get().set_value("shortcuts", &map.to_variant());
}

/// Every entry with the keys in effect: (id, action, param, keys, defaults, description).
/// One shortcut entry with the keys in effect.
pub struct Accel {
    /// "win.tidy", or "win.view-mode('list')".
    pub id: String,
    pub action: &'static str,
    pub param: &'static str,
    pub keys: Vec<String>,
    pub defaults: Vec<String>,
    pub description: &'static str,
}

pub fn effective() -> Vec<Accel> {
    let changes = overrides();
    ACCELS
        .iter()
        .map(|(action, param, keys, description)| {
            let id = accel_id(action, param);
            let defaults: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
            let keys = changes
                .get(&id)
                .cloned()
                .unwrap_or_else(|| defaults.clone());
            Accel {
                id,
                action,
                param,
                keys,
                defaults,
                description,
            }
        })
        .collect()
}

/// Comparable form of a key sequence ("ctrl+shift+o" for "Shift+Ctrl+O").
fn normal(key: &str) -> String {
    let lower = key.trim().to_lowercase();
    let mut parts: Vec<&str> = lower.split('+').collect();
    // "Ctrl++" ends in an empty part: the key is "+".
    let last = if lower.ends_with("++") {
        "+"
    } else {
        parts.pop().unwrap_or("")
    };
    parts.retain(|p| !p.is_empty() && *p != last);
    parts.sort_unstable();
    parts.push(last);
    parts.join("+")
}

/// Gives an entry a new key. Refused, with the reason, when another entry
/// already uses that key.
pub fn set_shortcut(id: &str, key: &str) -> Result<(), String> {
    let all = effective();
    let Some(entry) = all.iter().find(|e| e.id == id) else {
        return Err(String::new());
    };
    let wanted = normal(key);
    if let Some(other) = all
        .iter()
        .find(|e| e.id != id && e.keys.iter().any(|k| normal(k) == wanted))
    {
        return Err(format!(
            "{} {key} {} “{}”",
            gettextrs::gettext("The shortcut"),
            gettextrs::gettext("is already used by"),
            gettextrs::gettext(other.description)
        ));
    }
    let mut changes = overrides();
    if entry.defaults == [key.to_string()] {
        changes.remove(id);
    } else {
        changes.insert(id.to_string(), vec![key.to_string()]);
    }
    set_overrides(&changes);
    Ok(())
}

/// Takes an entry's shortcut away.
pub fn clear_shortcut(id: &str) {
    let mut changes = overrides();
    changes.insert(id.to_string(), Vec::new());
    set_overrides(&changes);
}

/// Every shortcut back to the built-in one.
pub fn reset_shortcuts() {
    settings::get().reset("shortcuts");
}

pub fn accels_json() -> Value {
    Value::Array(
        effective()
            .into_iter()
            .map(|a| {
                json!({ "id": a.id, "action": a.action, "param": a.param,
                        "custom": a.keys != a.defaults, "keys": a.keys,
                        "description": gettextrs::gettext(a.description),
                        "typing": TEXT_KEYS.contains(&a.action) })
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
        glib::g_warning!("steward", "no action {detailed}");
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
    // The Shelf (P8-T01).
    simple(&app, "shelf", || controller::with(|c| c.shelf_toggle()));
    simple(&app, "report-bug", || {
        let url = crate::report::open();
        controller::with(|c| c.reported(url));
    });
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
    // Search inside files (P7-T09).
    toggle(
        &win,
        "search-contents",
        s.boolean("search-contents"),
        |on| {
            let _ = settings::get().set_boolean("search-contents", on);
            controller::with(|c| c.search_contents(on))
        },
    );
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
    with_string(&win, "tool-open", |page| {
        controller::with(|c| c.open_tool(page))
    });
    simple(&win, "tool-close", || controller::with(|c| c.close_tool()));
    with_string(&win, "clean-review", |id| {
        controller::with(|c| c.clean_review(id))
    });
    simple(&win, "clean-look-again", || {
        controller::with(|c| c.clean_look_again())
    });
    with_string(&win, "clean-tick", |what| {
        controller::with(|c| c.clean_tick(what))
    });
    with_string(&win, "clean-trash", |id| {
        controller::with(|c| c.clean_trash(id))
    });
    with_string(&win, "clean-copy", |id| {
        controller::with(|c| c.clean_copy(id))
    });
    with_string(&win, "clean-show", |path| {
        controller::with(|c| c.clean_show(path))
    });
    with_string(&win, "clean-peek", |path| {
        controller::with(|c| c.clean_peek(path))
    });
    with_string(&win, "agents-period", |period| {
        controller::with(|c| c.agents_period(period))
    });
    with_string(&win, "agents-stop-ask", |pid| {
        controller::with(|c| c.agents_stop_ask(pid))
    });
    with_string(&win, "agents-stop", |answer| {
        controller::with(|c| c.agents_stop(answer == "yes"))
    });
    with_string(&win, "agents-open", |port| {
        controller::with(|c| c.agents_open(port))
    });
    simple(&win, "agents-refresh", || {
        controller::with(|c| c.agents_refresh())
    });
    simple(&win, "shelf-close", || {
        controller::with(|c| c.shelf_close())
    });
    simple(&win, "present", || controller::with(|c| c.present()));
    simple(&win, "close-window", || {
        controller::with(|c| c.close_window())
    });
    with_string(&win, "shelf-open-pin", |n| {
        controller::with(|c| c.shelf_open_pin(n))
    });
    with_string(&win, "shelf-open", |path| {
        controller::with(|c| c.shelf_open(path))
    });
    with_string(&win, "shelf-pin", |path| {
        controller::with(|c| c.shelf_pin(path))
    });
    with_string(&win, "shelf-unpin", |n| {
        controller::with(|c| c.shelf_unpin(n))
    });
    with_string(&win, "shelf-hold", |list| {
        controller::with(|c| c.shelf_hold(list))
    });
    with_string(&win, "shelf-unhold", |id| {
        controller::with(|c| c.shelf_unhold(id))
    });
    simple(&win, "shelf-clear", || {
        controller::with(|c| c.shelf_clear())
    });
    with_string(&win, "shelf-drop", |v| {
        controller::with(|c| c.shelf_drop(v))
    });
    simple(&win, "shelf-undo", || controller::with(|c| c.shelf_undo()));
    // Customisable shortcuts (P7-T11): "id|key", id, and all.
    with_string(&win, "shortcut-set", |value| {
        let Some((id, key)) = value.rsplit_once('|') else {
            return;
        };
        if let Err(reason) = set_shortcut(id, key)
            && !reason.is_empty()
        {
            controller::with(|c| c.toast(&reason));
        }
    });
    with_string(&win, "shortcut-clear", clear_shortcut);
    simple(&win, "shortcuts-reset", reset_shortcuts);
    with_string(&win, "md-mode", |mode| {
        controller::with(|c| c.md_mode(mode))
    });
    with_string(&win, "md-edit", |text| {
        controller::with(|c| c.md_edit(text))
    });
    with_string(&win, "md-link", |link| {
        controller::with(|c| c.md_link(link))
    });
    simple(&win, "md-save", || controller::with(|c| c.md_save()));
    with_string(&win, "bookmarks-show", |f| {
        controller::with(|c| c.bookmarks_show(f))
    });
    with_string(&win, "bookmarks-tag", |t| {
        controller::with(|c| c.bookmarks_tag(t))
    });
    with_string(&win, "bookmarks-view", |v| {
        controller::with(|c| c.bookmarks_view(v))
    });
    with_string(&win, "bookmark-add", |url| {
        controller::with(|c| c.bookmark_add(url))
    });
    simple(&win, "bookmark-add-ask", || {
        controller::with(|c| c.bookmark_add_ask())
    });
    with_string(&win, "bookmark-open", |id| {
        controller::with(|c| c.bookmark_open(id))
    });
    with_string(&win, "bookmark-copy", |id| {
        controller::with(|c| c.bookmark_copy(id))
    });
    with_string(&win, "bookmark-favourite", |id| {
        controller::with(|c| c.bookmark_favourite(id))
    });
    with_string(&win, "bookmark-delete", |id| {
        controller::with(|c| c.bookmark_delete(id))
    });
    with_string(&win, "bookmark-edit-ask", |id| {
        controller::with(|c| c.bookmark_edit_ask(id))
    });
    with_string(&win, "bookmark-edit", |v| {
        controller::with(|c| c.bookmark_edit(v))
    });
    simple(&win, "collection-new", || {
        controller::with(|c| c.collection_new_ask())
    });
    with_string(&win, "collection-create", |n| {
        controller::with(|c| c.collection_create(n))
    });
    with_string(&win, "collection-delete", |id| {
        controller::with(|c| c.collection_delete(id))
    });
    simple(&win, "md-revert", || controller::with(|c| c.md_revert()));
    with_string(&win, "open-terminal", |path| {
        controller::with(|c| c.open_in(steward_core::openin::Role::Terminal, path))
    });
    with_string(&win, "open-editor", |path| {
        controller::with(|c| c.open_in(steward_core::openin::Role::Editor, path))
    });
    simple(&win, "openin-rescan", || {
        controller::with(|c| c.openin_rescan())
    });
    with_string(&win, "installed-filter", |id| {
        controller::with(|c| c.installed_filter(id))
    });
    with_string(&win, "installed-tool", |id| {
        controller::with(|c| c.installed_tool(id))
    });
    with_string(&win, "installed-copy", |what| {
        controller::with(|c| c.installed_copy(what))
    });
    with_string(&win, "installed-save", |path| {
        controller::with(|c| c.installed_save(path))
    });
    simple(&win, "installed-refresh", || {
        controller::with(|c| c.installed_refresh())
    });
    with_string(&win, "about-hide-names", |hide| {
        controller::with(|c| c.about_hide_names(hide == "true"))
    });
    simple(&win, "about-copy", || controller::with(|c| c.about_copy()));
    with_string(&win, "welcome-step", |change| {
        controller::with(|c| c.welcome_step(change))
    });
    simple(&win, "welcome-done", || {
        controller::with(|c| c.first_run_done())
    });
    simple(&win, "workspace-new", || {
        controller::with(|c| c.ask_new_workspace())
    });
    with_string(&win, "workspace-create", |name| {
        controller::with(|c| c.create_workspace(name))
    });
    with_string(&win, "workspace-open", |arg| {
        controller::with(|c| c.open_workspace_arg(arg))
    });
    simple(&win, "workspace-close", || {
        controller::with(|c| c.close_workspace())
    });
    with_string(&win, "workspace-tab", |tab| {
        controller::with(|c| c.workspace_tab(tab))
    });
    with_string(&win, "workspace-add", |arg| {
        controller::with(|c| c.add_to_workspace_arg(arg))
    });
    with_string(&win, "workspace-remove", |item| {
        controller::with(|c| c.remove_from_workspace(item))
    });
    with_string(&win, "workspace-locate", |arg| {
        controller::with(|c| c.locate_item(arg))
    });
    with_string(&win, "workspace-edit-ask", |id| {
        controller::with(|c| c.ask_edit_workspace(id))
    });
    with_string(&win, "workspace-edit", |arg| {
        controller::with(|c| c.edit_workspace(arg))
    });
    with_string(&win, "workspace-delete", |id| {
        controller::with(|c| c.delete_workspace(id))
    });
    simple(&win, "folder-style-ask", || {
        controller::with(|c| c.ask_folder_style())
    });
    with_string(&win, "folder-style", |arg| {
        controller::with(|c| c.set_folder_style(arg))
    });
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
    simple(&win, "extract-here", || {
        controller::with(|c| c.extract_here())
    });
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
    with_string(&win, "sidebar-show", |key| {
        controller::with(|c| c.sidebar_show(key))
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
