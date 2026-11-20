"""Phase 4 end-to-end scenarios: Smart Downloads (DESIGN.md §10, Phase 4)."""

import os
from pathlib import Path

import pytest
from gi.repository import GLib

pytestmark = pytest.mark.p4

TITLE = "Tidy “Downloads”"


def s(text: str) -> GLib.Variant:
    return GLib.Variant("s", text)


def open_plan(app):
    app.press("ctrl+shift+o")
    app.wait_for(lambda: (d := app.state()["active_window"]["dialog"]) and d["type"] == "tidy", "the Tidy plan")
    return app.state()["tidy"]


def rel_moves(home, plan) -> list[tuple[str, str]]:
    return sorted((str(Path(m["from"]).relative_to(home.path)), str(Path(m["to"]).relative_to(home.path)))
                  for m in plan["moves"])


def manifest_moves(home, leave_out=()) -> list[tuple[str, str]]:
    return sorted((m["from"], m["to"]) for m in home.manifest["tidy"]["moves"]
                  if m["from"] not in leave_out)


def test_E4_01_the_plan(app, home):
    touched = "Downloads/Budget-Q4.ods"
    os.utime(home / touched, None)  # "a moment ago"
    app.open_folder(home / "Downloads")
    plan = open_plan(app)
    assert rel_moves(home, plan) == manifest_moves(home, leave_out=[touched])
    left = {item["name"]: item["reason"] for item in plan["left"]}
    for name in home.manifest["tidy"]["left"]:
        assert left[name] == "Downloading", name
    assert left["Budget-Q4.ods"] == "Recent"
    assert plan["folders_moved"] == 0, "no folder is listed"


def test_E4_02_cancel_changes_nothing(app, home):
    app.open_folder(home / "Downloads")
    before = home.snapshot("Downloads")
    open_plan(app)
    app.a11y_click("Cancel", role="button", within=TITLE)
    app.wait_for(lambda: app.state()["active_window"]["dialog"] is None, "the plan to close")
    app.wait_idle()
    assert home.snapshot("Downloads") == before


def test_E4_03_tidy_then_one_undo(app, home):
    app.open_folder(home / "Downloads")
    before = home.snapshot("Downloads")
    open_plan(app)
    app.a11y_click("Tidy", role="button", within=TITLE)
    app.wait_idle()
    for move in home.manifest["tidy"]["moves"]:
        assert (home / move["to"]).exists(), move["to"]
        assert not (home / move["from"]).exists(), move["from"]
    moves = len(home.manifest["tidy"]["moves"])
    folders = len({Path(m["to"]).parent for m in home.manifest["tidy"]["moves"]})
    assert app.state()["last_toast"] == f"Tidied {moves} files into {folders} folders"

    app.press("ctrl+z")
    app.wait_idle()
    assert home.snapshot("Downloads") == before, "exactly as before, kind folders gone"


def test_E4_04_tidy_twice(app, home):
    app.open_folder(home / "Downloads")
    open_plan(app)
    app.a11y_click("Tidy", role="button", within=TITLE)
    app.wait_idle()
    plan = open_plan(app)
    assert plan["moves"] == []
    app.a11y_click("Close", role="button", within=TITLE)


def test_E4_05_a_name_already_in_the_kind_folder(app, home):
    (home / "Downloads" / "PDFs").mkdir()
    (home / "Downloads" / "PDFs" / "Statement-0923.pdf").write_text("an older statement\n")
    original = (home / "Downloads" / "Statement-0923.pdf").read_bytes()
    app.open_folder(home / "Downloads")
    before = home.snapshot("Downloads")
    open_plan(app)
    app.a11y_click("Tidy", role="button", within=TITLE)
    app.wait_idle()
    assert (home / "Downloads" / "PDFs" / "Statement-0923 (2).pdf").read_bytes() == original
    assert (home / "Downloads" / "PDFs" / "Statement-0923.pdf").read_text() == "an older statement\n"
    app.press("ctrl+z")
    app.wait_idle()
    assert home.snapshot("Downloads") == before
