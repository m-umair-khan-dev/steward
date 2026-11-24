"""Phase 10: the UI used with the real mouse, as a person would.

Accessibility actions call a control's handler directly; a real click goes
through Qt's pointer delivery, where a handler underneath can also react
(a tap on a Preferences tab used to reach the dialog's backdrop and close
the dialog). These scenarios click with the pointer and check that the app
is still running, the window or dialog is still open, and the click did
what its label says.
"""

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

