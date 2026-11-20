"""E6-04: accessibility audit over every main screen (P6-T05).

Every showing control a person can use has a name and an action a screen
reader can trigger, and the shortcuts window lists every bound key."""

import pytest
from gi.repository import GLib

pytestmark = pytest.mark.p6

INTERACTIVE = {"button", "toggle button", "check box", "radio button", "menu item", "link", "page tab",
               "check menu item", "radio menu item", "combo box", "slider", "spin button"}
TEXT_FIELDS = {"entry", "text", "password text"}


def s(text: str) -> GLib.Variant:
    return GLib.Variant("s", text)


def problems(tree: dict, screen: str) -> list[str]:
    found = []

    def walk(node: dict, path: str) -> None:
        if not node["showing"]:
            return
        role, name = node["role"], node["name"].strip()
        here = f"{path} > {role} {name!r}"
        if role in INTERACTIVE | TEXT_FIELDS and not name:
            found.append(f"{screen}: a {role} without a name ({path})")
        if role in INTERACTIVE and node["enabled"] and not node["actions"]:
            found.append(f"{screen}: {role} {name!r} has no action")
        for child in node["children"]:
            walk(child, here if name else path)

    walk(tree, "app")
    return found


def audit(app, screen: str, found: list[str]) -> None:
    app.wait_idle()
    found += problems(app.a11y_tree(), screen)


def test_E6_04_every_screen_is_accessible(app, home):
    found: list[str] = []
    app.open_folder(home / "Downloads")
    audit(app, "folder grid", found)
    app.action("win.toggle-preview")
    app.select("notes.txt")
    audit(app, "preview pane", found)
    app.action("win.toggle-preview")
    app.action("win.view-mode", s("list"))
    audit(app, "folder list", found)
    app.action("win.view-mode", s("grid"))
    app.press("ctrl+k")
    app.wait_for(lambda: app.state()["search"]["open"], "search")
    app.type_text("invo")
    audit(app, "search", found)
    app.press("escape")
    app.select("notes.txt")
    app.press("space")
    app.wait_for(lambda: app.state()["peek"]["open"], "Peek")
    audit(app, "peek", found)
    app.press("escape")
    app.press("f3")
    app.wait_for(lambda: app.state()["pane"]["open"], "dual pane")
    audit(app, "dual pane", found)
    app.press("f3")
    app.action("win.tidy")
    app.wait_for(lambda: app.state()["active_window"]["dialog"], "the Tidy plan")
    audit(app, "tidy plan", found)
    app.action("win.tidy-cancel")
    app.action("win.workspace-create", s("Taxes"))
    app.wait_for(lambda: app.state()["workspace"]["open"], "a workspace")
    audit(app, "workspace", found)
    app.action("win.workspace-close")
    app.action("win.tool-open", s("clean"))
    app.wait_for(lambda: (c := app.state()["clean"]) and c["cards"] and not c["looking"], "Free Up Space", 60)
    audit(app, "free up space", found)
    app.action("win.clean-review", s("duplicates"))
    audit(app, "review", found)
    app.action("win.tool-open", s("installed"))
    app.wait_for(lambda: app.state()["installed"]["loaded"], "Installed", 60)
    audit(app, "installed", found)
    app.action("win.tool-open", s("about"))
    app.wait_for(lambda: app.state()["about"]["loaded"], "About", 20)
    audit(app, "about", found)
    app.action("win.tool-close")
    app.action("app.preferences")
    app.wait_for(lambda: app.state()["active_window"]["dialog"], "Preferences")
    for page in ("General", "Appearance", "Shortcuts", "Search and Index"):
        app.a11y_click(page, within="Preferences")
        audit(app, f"preferences {page}", found)
    app.press("escape")
    app.open_folder(home / ".local" / "share" / "Trash" / "files")
    audit(app, "trash", found)
    assert not found, "\n".join(found)


def test_E6_04_the_shortcuts_window_lists_every_key(app):
    app.action("win.show-help-overlay")
    app.wait_for(lambda: app.state()["active_window"]["dialog"], "the shortcuts window")
    listed = app.a11y_names(showing_only=False)
    for action, keys in app.state()["accels"].items():
        for key in keys:
            assert any(key in name for name in listed), f"{action}: {key} is not listed"
