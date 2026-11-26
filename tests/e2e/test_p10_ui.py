"""Phase 10: the UI used with the real mouse, as a person would.

Accessibility actions call a control's handler directly; a real click goes
through Qt's pointer delivery, where a handler underneath can also react
(a tap on a Preferences tab used to reach the dialog's backdrop and close
the dialog). These scenarios click with the pointer and check that the app
is still running, the window or dialog is still open, and the click did
what its label says.
"""

import pytest

from harness.wait import wait_for

# Each Preferences page and a control only that page shows.
PREFERENCE_PAGES = {
    "General": "Show hidden files",
    "Appearance": "Card size",  # "Colour" hides while the system accent is used
    "Shortcuts": "Find a shortcut",
    "Shelf and Tray": "Open the Shelf with a key",
    "Search and Index": "Pause indexing",
}


def dialog(app):
    window = app.state()["active_window"]
    return window["dialog"]["type"] if window["dialog"] else None


def open_preferences(app):
    app.action("app.preferences")
    wait_for(lambda: dialog(app) == "preferences", "Preferences to open")


def test_E10_01_preferences_tabs_with_the_mouse(app):
    open_preferences(app)
    for tab, marker in [*list(PREFERENCE_PAGES.items())[1:], ("General", "Show hidden files")]:
        app.mouse_click(tab, role="button", within="Preferences pages")
        wait_for(lambda m=marker: app.find_node(m) is not None, f"the {tab} page")
        assert app.running(), f"the app quit after clicking {tab}"
        assert dialog(app) == "preferences", f"Preferences closed when {tab} was clicked"



# --- E10-03: each control does what its label says (real pointer) ------------

def _home_path(app, *parts):
    return str(app.home.path.joinpath(*parts))


def _pref_page(app, tab):
    open_preferences(app)
    app.mouse_click(tab, role="button", within="Preferences pages")
    wait_for(lambda: app.find_node(PREFERENCE_PAGES[tab]) is not None, f"the {tab} page")


def _menu(app, item, opener=None, right_click=None):
    """Opens a menu with the pointer and clicks one of its items."""
    if opener:
        app.mouse_click(opener, role="button")
    else:
        app.mouse_click(right_click, role="button", button="right")
    app.mouse_click(item, role="menu item")


def _first_file_on_screen(app):
    tree = app.a11y_tree()
    return next(n for n in app.folder()["items"]
                if "." in n and app.find_node(n, role="button", tree=tree))


def _restart(app_factory, app):
    app.stop()
    again = app_factory()
    return again


@pytest.mark.parametrize("setting", ["show hidden", "dark style", "large cards", "list by default"])
def test_E10_03_preferences_change_and_persist(setting, app_factory):
    tab, control, within, check = {
        "show hidden": ("General", "Show hidden files", None,
                        lambda s: s["folder"]["show_hidden"]),
        "dark style": ("Appearance", "Dark", "Style",
                       lambda s: s["appearance"]["style"] == "dark" and s["dark"]),
        "large cards": ("Appearance", "Large", "Card size",
                        lambda s: s["folder"]["row_size"] == "large"),
        "list by default": ("General", "List", "Default view", None),
    }[setting]
    app = app_factory()
    _pref_page(app, tab)
    app.mouse_click(control, within=within)
    if check:
        wait_for(lambda: check(app.state()), f"{setting} to take effect")
    assert app.state()["dialog"] == "preferences", "Preferences stays open"
    again = _restart(app_factory, app)
    final = check or (lambda s: s["folder"]["view_mode"] == "list")
    wait_for(lambda: final(again.state()), f"{setting} to be kept after a restart")


def test_E10_03_pause_indexing(app):
    app.wait_index_idle()
    _pref_page(app, "Search and Index")
    app.mouse_click("Pause indexing")
    wait_for(lambda: app.find_node("Indexing is paused.") is not None, "indexing to pause")
    app.mouse_click("Pause indexing")
    wait_for(lambda: app.find_node("Indexing is paused.") is None, "indexing to resume")


def test_E10_03_header_view_and_sort(app):
    app.open_folder(app.home.path / "Downloads")
    app.mouse_click("List", role="button", within="View")
    wait_for(lambda: app.folder()["view_mode"] == "list", "the list view")
    app.mouse_click("Grid", role="button", within="View")
    wait_for(lambda: app.folder()["view_mode"] == "grid", "the grid view")
    _menu(app, "Size", opener="Sort and size")
    wait_for(lambda: app.folder()["sort"] == "size", "sorting by size")
    _menu(app, "Descending", opener="Sort and size")
    wait_for(lambda: app.folder()["descending"], "descending order")
    _menu(app, "Larger", opener="Sort and size")
    wait_for(lambda: app.folder()["row_size"] == "large", "larger cards")


def test_E10_03_sidebar_and_navigation(app):
    app.open_folder(app.home.path)
    app.mouse_click("Documents", role="button", within="Sidebar")
    wait_for(lambda: app.folder()["path"] == _home_path(app, "Documents"), "Documents")
    app.mouse_click("Back", role="button")
    wait_for(lambda: app.folder()["path"] == _home_path(app), "back to Home")
    app.mouse_click("Forward", role="button")
    wait_for(lambda: app.folder()["path"] == _home_path(app, "Documents"), "forward to Documents")
    # At 800 pixels high the Tools list runs under the Computer row at the
    # bottom of the sidebar: scroll down first, as a person would.
    x, y, w, h = app.find_node("Sidebar", role="list")["extents"]
    app.scroll_at(x + w / 2, y + h / 2, 10)
    for name, page in (("Free Up Space", "clean"), ("Installed", "installed"), ("AI Agents", "agents")):
        app.mouse_click(name, role="button", within="Tools")
        wait_for(lambda p=page: app.state()["tools"]["page"] == p, f"the {name} page")
    app.mouse_click("Collapse sidebar", role="button")
    wait_for(lambda: not app.state()["sidebar"]["expanded"], "the sidebar to collapse")


def test_E10_03_background_menu(app):
    folder = app.home.path / "Documents"
    app.open_folder(folder)
    before = {p.name for p in folder.iterdir()}
    app.click_at(900, 760, button="right")
    app.mouse_click("New Folder", role="menu item")
    wait_for(lambda: len({p.name for p in folder.iterdir()} - before) == 1, "a new folder")
    app.press("escape")  # leave the name as it is
    app.wait_idle()
    app.click_at(900, 760, button="right")
    app.mouse_click("Select All", role="menu item")
    wait_for(lambda: set(app.folder()["selection"]) == set(app.folder()["items"]), "everything selected")
    app.click_at(900, 760, button="right")
    app.mouse_click("Show Hidden Files", role="menu item")
    wait_for(lambda: app.folder()["show_hidden"], "hidden files shown")


def test_E10_03_item_menu(app):
    folder = app.home.path / "Downloads"
    app.open_folder(folder)
    name = _first_file_on_screen(app)
    count = len(list(folder.iterdir()))
    _menu(app, "Duplicate", right_click=name)
    wait_for(lambda: len(list(folder.iterdir())) == count + 1, "the duplicate")
    _menu(app, "Copy", right_click=name)
    wait_for(lambda: app.state()["ops"]["canPaste"], "something to paste")
    _menu(app, "Move to Trash", right_click=name)
    wait_for(lambda: not (folder / name).exists(), f"{name} in the Trash")
    assert name in [p.name for p in (app.home.path / ".local/share/Trash/files").iterdir()]
    app.mouse_click("Main menu", role="button")
    undo = wait_for(lambda: next((n for n in _menu_items(app) if n.startswith("Undo")), None), "Undo")
    app.mouse_click(undo, role="menu item")
    wait_for(lambda: (folder / name).exists(), f"{name} back after Undo")


def _menu_items(app):
    menu = app.find_node_role(("menu", "popup menu"))
    out = []
    stack = [menu] if menu else []
    while stack:
        node = stack.pop()
        if node["role"] == "menu item" and node["enabled"]:
            out.append(node["name"])
        stack.extend(node["children"])
    return out


@pytest.mark.parametrize("item,kind", [("Preferences", "preferences"), ("Keyboard Shortcuts", "shortcuts"),
                                       ("About Steward", "about")])
def test_E10_03_main_menu_opens_dialogs_and_close_closes_them(item, kind, app):
    _menu(app, item, opener="Main menu")
    wait_for(lambda: dialog(app) == kind, f"the {kind} dialog")
    app.mouse_click("Close", role="button")
    wait_for(lambda: dialog(app) is None, f"the {kind} dialog to close")


def test_E10_03_main_menu_style(app):
    _menu(app, "Dark", opener="Main menu")
    wait_for(lambda: app.state()["dark"], "the dark style")
    _menu(app, "Light", opener="Main menu")
    wait_for(lambda: not app.state()["dark"], "the light style")
