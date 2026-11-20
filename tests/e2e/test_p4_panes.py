"""Phase 4 end-to-end scenarios: dual pane (DESIGN.md §10, Phase 4)."""

import pytest
from gi.repository import GLib

pytestmark = pytest.mark.p4


def s(text: str) -> GLib.Variant:
    return GLib.Variant("s", text)


def pane(app) -> dict:
    return app.state()["pane"]


def test_E4_09_copy_and_move_across(app, home):
    app.open_folder(home / "Downloads")
    app.press("f3")
    app.wait_for(lambda: pane(app)["open"], "two panes")
    target = home / "Documents" / "Invoices"
    app.action("win.pane-go", s(str(target)))
    app.wait_for(lambda: pane(app)["path"] == str(target) and pane(app)["items"], "the right pane's folder")

    app.select("notes.txt")
    app.press("f5")
    app.wait_idle()
    assert (target / "notes.txt").exists() and (home / "Downloads" / "notes.txt").exists(), "copied"
    app.wait_for(lambda: "notes.txt" in pane(app)["items"], "the copy in the right pane")

    app.select("export.csv")
    app.press("f6")
    app.wait_idle()
    assert (target / "export.csv").exists() and not (home / "Downloads" / "export.csv").exists(), "moved"

    app.press("ctrl+z")
    app.wait_idle()
    assert (home / "Downloads" / "export.csv").exists() and not (target / "export.csv").exists(), \
        "Undo reverts the move"

    assert not pane(app)["right_active"]
    app.press("tab")
    app.wait_for(lambda: pane(app)["right_active"], "the right pane to become active")
    app.press("tab")
    app.wait_for(lambda: not pane(app)["right_active"], "the left pane again")


def test_E4_10_panes_come_back_after_a_restart(session, home, app_factory):
    app = app_factory()
    app.open_folder(home / "Pictures")
    app.press("f3")
    app.wait_for(lambda: pane(app)["open"], "two panes")
    app.action("win.pane-go", s(str(home / "Documents")))
    app.wait_for(lambda: pane(app)["path"] == str(home / "Documents"), "the right pane's folder")
    app.wait_idle()
    app.stop()

    again = app_factory()
    again.wait_for(lambda: pane(again)["open"], "two panes after the restart")
    again.wait_for(lambda: again.folder()["path"] == str(home / "Pictures"), "the left folder")
    assert pane(again)["path"] == str(home / "Documents")
