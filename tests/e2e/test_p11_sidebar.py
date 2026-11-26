"""Phase 11: the sidebar (GitHub issue #4), with the real pointer.

A Favorites entry hidden from its right-click menu says where to bring it
back and offers Undo; Preferences › General lists every entry with a switch
and restores the sidebar; the slide-out sidebar can be kept open.
"""

from harness.wait import wait_for


def labels(app):
    return [e["label"] for e in app.state()["sidebar"]["entries"]]


def hide_from_menu(app, label):
    app.mouse_click(label, role="button", within="Sidebar", button="right")
    app.mouse_click("Hide", role="menu item")
    wait_for(lambda: label not in labels(app), f"{label} to leave the sidebar")


def general_preferences(app):
    app.action("app.preferences")
    wait_for(lambda: app.state()["dialog"] == "preferences", "Preferences")
    # The Sidebar section is at the end of General: scroll down to it.
    x, y, w, h = app.find_node("Preferences", role="dialog")["extents"]
    app.scroll_at(x + w / 2, y + h / 2, 15)


def test_E11_01_hidden_entry_toast_says_where_and_undo_brings_it_back(app):
    hide_from_menu(app, "Music")
    toast = wait_for(lambda: app.state()["last_toast"], "the toast")
    assert "Music hidden" in toast and "Preferences › General" in toast
    app.mouse_click("Undo", role="button")
    wait_for(lambda: "Music" in labels(app), "Music back after Undo")


def test_E11_02_preferences_switch_brings_one_entry_back(app):
    hide_from_menu(app, "Music")
    hide_from_menu(app, "Videos")
    general_preferences(app)
    app.mouse_click("Show in sidebar: Music")
    wait_for(lambda: "Music" in labels(app), "Music back from Preferences")
    assert "Videos" not in labels(app), "only the one switched on comes back"
    assert app.state()["dialog"] == "preferences", "Preferences stays open"
    # And off again from Preferences.
    app.mouse_click("Show in sidebar: Home")
    wait_for(lambda: "Home" not in labels(app), "Home hidden from Preferences")


def test_E11_03_restore_sidebar_in_preferences(app):
    hide_from_menu(app, "Music")
    app.mouse_click("Documents", role="button", within="Sidebar", button="right")
    app.mouse_click("Move Up", role="menu item")
    wait_for(lambda: labels(app).index("Documents") < labels(app).index("Desktop"), "the new order")
    general_preferences(app)
    app.mouse_click("Restore Sidebar", role="button")
    wait_for(lambda: "Music" in labels(app)
             and labels(app).index("Desktop") < labels(app).index("Documents"), "the default sidebar")


def test_E11_04_slide_out_sidebar_can_be_kept_open(app):
    app.mouse_click("Collapse sidebar", role="button")
    wait_for(lambda: not app.state()["sidebar"]["expanded"], "the sidebar to collapse")
    # Point at the rail: the sidebar slides out, with a button to keep it.
    app.point_at(20, 400)
    app.mouse_click("Keep sidebar open", role="button", timeout=10)
    wait_for(lambda: app.state()["sidebar"]["expanded"], "the sidebar to stay open")


def test_E11_05_keep_sidebar_open_switch_in_preferences(app):
    general_preferences(app)
    app.mouse_click("Keep sidebar open")
    wait_for(lambda: not app.state()["sidebar"]["expanded"], "the sidebar to hide")
    app.mouse_click("Keep sidebar open")
    wait_for(lambda: app.state()["sidebar"]["expanded"], "the sidebar to stay open")
