"""E10-02: every control on every screen, clicked with the real pointer.

Each screen runs in its own app with a fresh fixture home. The sweep
(harness/sweep.py) reports every click that crashed the app, hung it,
logged a QML error, or closed a dialog by changing one of its settings.
Reports: artifacts/e2e/sweep/<screen>.json.
"""

from pathlib import Path

import pytest
from gi.repository import GLib

from harness.sweep import Sweep, summary
from harness.wait import wait_for

REPORTS = Path(__file__).resolve().parents[2] / "artifacts" / "e2e" / "sweep"


def s(text: str) -> GLib.Variant:
    return GLib.Variant("s", text)


def dialog_open(app, kind: str) -> None:
    wait_for(lambda: app.state()["dialog"] == kind, f"the {kind} dialog")


def with_dialog(action: str, kind: str, before=None, param=None):
    def setup(app):
        if before:
            before(app)
        if app.state()["dialog"] != kind:
            app.action(action, param)
            dialog_open(app, kind)
    return setup


def defaults(app) -> None:
    """Undoes the saved layout changes earlier clicks may have made (they
    live in the fixture home's settings and survive a restart)."""
    state = app.state()
    if not state["sidebar"]["expanded"]:
        app.action("win.toggle-sidebar")
        wait_for(lambda: app.state()["sidebar"]["expanded"], "the sidebar")
    if state["folder"]["view_mode"] != "grid":
        app.action("win.view-mode", s("grid"))
        wait_for(lambda: app.folder()["view_mode"] == "grid", "the grid view")


def in_folder(*parts: str, select: str | None = None):
    def setup(app):
        defaults(app)
        path = app.home.path.joinpath(*parts)
        if app.folder()["path"] != str(path):
            app.open_folder(path)
        # At start-up the app is already in the home folder, but its listing
        # may still be loading (slow CI runners): a select before the item is
        # listed does nothing. Wait for the listing, and select until it holds.
        wait_for(lambda: (f := app.folder())["state"] != "loading"
                 and (not select or select in f["items"]), f"{path.name or 'home'} to be listed", 15)

        def selected():
            if app.folder()["selection"] == [select]:
                return True
            app.select(select)
            return False

        if select:
            wait_for(selected, f"{select} selected", 10)
    return setup


def text_apps(home) -> None:
    """Two harmless apps for text files, so Open With has a choice. Written
    before the app starts: GIO reads the list of apps once."""
    folder = home.path / ".local" / "share" / "applications"
    folder.mkdir(parents=True, exist_ok=True)
    for name in ("Fake Editor", "Fake Viewer"):
        (folder / f"{name.replace(' ', '-').lower()}.desktop").write_text(
            f"[Desktop Entry]\nType=Application\nName={name}\nExec=true %f\n"
            "MimeType=text/plain;\n")
    # GIO matches apps to types through this cache (packages keep it current).
    (folder / "mimeinfo.cache").write_text(
        "[MIME Cache]\ntext/plain=fake-editor.desktop;fake-viewer.desktop;\n")


def tool(page: str):
    def setup(app):
        in_folder()(app)
        if app.state()["tools"]["page"] != page:
            app.action("win.tool-open", s(page))
            wait_for(lambda: app.state()["tools"]["page"] == page, f"the {page} page")
    return setup


def preferences_tab(tab: str):
    def setup(app):
        with_dialog("app.preferences", "preferences")(app)
        app.a11y_click(tab, role="button", within="Preferences pages")
    return setup


def toggled(action: str, key: str):
    def setup(app):
        in_folder("Downloads")(app)
        state = app.state()
        part, _, field = key.partition(".")
        if not state[part][field]:
            app.action(action)
            wait_for(lambda: app.state()[part][field], f"{key}")
    return setup


def menu_on_item(app):
    """The right-click menu of the first file on screen in Downloads (a menu
    item such as Move to Trash may have moved the last one away)."""
    in_folder("Downloads")(app)
    tree = app.a11y_tree()
    files = [n for n in app.folder()["items"] if "." in n]
    name = next(n for n in files if app.find_node(n, role="button", tree=tree))
    app.mouse_click(name, role="button", button="right")
    wait_for(lambda: app.find_node_role(("menu", "popup menu")), "the item menu")


def menu_on_background(app):
    in_folder("Documents")(app)
    # Below the last row of cards: the folder's empty background.
    app.click_at(900, 760, button="right")
    wait_for(lambda: app.find_node_role(("menu", "popup menu")), "the background menu")


def menu_from(button: str):
    def setup(app):
        in_folder("Downloads")(app)
        app.mouse_click(button, role="button")
        wait_for(lambda: app.find_node_role(("menu", "popup menu")), f"the {button} menu")
    return setup


# name: (setup, the dialog whose controls are clicked, or None for the window)
SCREENS = {
    "home folder": (in_folder(), None),
    "downloads with a file selected": (in_folder("Downloads", select="notes.txt"), None),
    "two panes": (toggled("win.dual-pane", "pane.open"), None),
    "search": (toggled("win.search", "search.open"), None),
    "peek": (lambda app: (in_folder("Downloads", select="notes.txt")(app), app.action("win.peek"),
                          wait_for(lambda: app.state()["peek"]["open"], "peek")), None),
    "installed": (tool("installed"), None),
    "free up space": (tool("clean"), None),
    "about this computer": (tool("about"), None),
    "bookmarks": (tool("bookmarks"), None),
    "agents": (tool("agents"), None),
    "preferences: general": (preferences_tab("General"), "Preferences"),
    "preferences: appearance": (preferences_tab("Appearance"), "Preferences"),
    "preferences: shortcuts": (preferences_tab("Shortcuts"), "Preferences"),
    "preferences: shelf and tray": (preferences_tab("Shelf and Tray"), "Preferences"),
    "preferences: search and index": (preferences_tab("Search and Index"), "Preferences"),
    "about dialog": (with_dialog("app.about", "about"), "About"),
    "keyboard shortcuts dialog": (with_dialog("win.show-help-overlay", "shortcuts"), "Keyboard Shortcuts"),
    "tidy dialog": (with_dialog("win.tidy", "tidy", before=in_folder("Downloads")), "Tidy “Downloads”"),
    "open with dialog": (with_dialog("win.open-with", "open-with",
                                     before=in_folder("Downloads", select="notes.txt")),
                         "Open With"),
    "symbol and colour dialog": (with_dialog("win.folder-style-ask", "folder-style",
                                             before=in_folder(select="Documents")), "Symbol and Colour"),
    "add link dialog": (with_dialog("win.bookmark-add-ask", "bookmark-add"), "Add Link"),
    "new workspace dialog": (with_dialog("win.workspace-new", "workspace-new"), "New Workspace"),
    "item menu": (menu_on_item, "@menu"),
    "background menu": (menu_on_background, "@menu"),
    "sort menu": (menu_from("Sort and size"), "@menu"),
    "main menu": (menu_from("Main menu"), "@menu"),
    "new collection dialog": (with_dialog("win.collection-new", "collection-new",
                                          before=tool("bookmarks")), "New Collection"),
}


@pytest.mark.parametrize("screen", list(SCREENS))
def test_E10_02_sweep(screen, app_factory, home):
    setup, within = SCREENS[screen]
    text_apps(home)
    result = Sweep(screen, app_factory, setup, within, REPORTS).run()
    print(summary(result))
    assert not result.failures, summary(result)
