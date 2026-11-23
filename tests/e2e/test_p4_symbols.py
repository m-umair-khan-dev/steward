"""Phase 4 end-to-end scenario E4-14: folder symbols and colours."""

import os

import pytest
from gi.repository import GLib

pytestmark = pytest.mark.p4


def s(text: str) -> GLib.Variant:
    return GLib.Variant("s", text)


def styles(app) -> dict:
    return app.state()["folder_styles"]


def _symbols_and_a_choice_that_survives_a_rename(app, home):
    # A Git project with a name that says nothing.
    (home / "Documents" / "School" / ".git").mkdir()
    app.open_folder(home / "Documents")
    found = app.wait_for(lambda: (st := styles(app)) and "Invoices" in st and st, "folder symbols")
    assert found["Invoices"]["symbol"] == "receipt"
    assert found["School"]["symbol"] == "code", "the Git project"
    assert found["Manuals"]["symbol"] == "pdf", "the mostly-PDF folder"

    app.select("Taxes 2026")
    app.action("win.folder-style-ask")
    app.wait_for(lambda: (d := app.state()["active_window"]["dialog"]) and d["type"] == "folder-style",
                 "the chooser")
    app.action("win.folder-style", s("plane|videos"))
    app.wait_for(lambda: styles(app).get("Taxes 2026") == {"symbol": "plane", "color": "videos"}, "the choice")

    app.press("f2")
    app.wait_for(lambda: app.state()["ops"]["renaming"], "the rename editor")
    app.type_text("Trips 2027")
    app.press("return")
    app.wait_for(lambda: (home / "Documents" / "Trips 2027").is_dir(), "the rename")
    app.wait_for(lambda: styles(app).get("Trips 2027") == {"symbol": "plane", "color": "videos"},
                 "the choice to survive the rename")
    return home / "Documents" / "Trips 2027"


def test_E4_14_folder_symbols(app, home):
    folder = _symbols_and_a_choice_that_survives_a_rename(app, home)
    assert os.getxattr(folder, "user.steward.symbol") == b"plane", "kept in an extended attribute"


def test_E4_14_folder_symbols_without_attributes(session, home, app_factory):
    app = app_factory(extra_env={"STEWARD_NO_XATTR": "1"})
    folder = _symbols_and_a_choice_that_survives_a_rename(app, home)
    with pytest.raises(OSError):
        os.getxattr(folder, "user.steward.symbol")
