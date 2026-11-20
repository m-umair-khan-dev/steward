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


def _fake_dpkg_query(folder: Path, version: str) -> None:
    """A stand-in for dpkg-query (DESIGN.md §10.2): reports photo-tool installed."""
    folder.mkdir(exist_ok=True)
    script = folder / "dpkg-query"
    script.write_text(f"#!/bin/sh\necho \"$@\" >> {folder}/calls.log\necho 'installed {version}'\n")
    script.chmod(0o755)


def test_E4_06_installer_for_an_installed_app(session, home, app_factory, tmp_path):
    fake = tmp_path / "fake-bin"
    _fake_dpkg_query(fake, "3.2.0")
    app = app_factory(extra_env={"PATH": f"{fake}:{session.base_env()['PATH']}"})
    app.open_folder(home / "Downloads")
    found = app.wait_for(lambda: (d := app.state()["downloads"]) and d["installers"] is not None
                         and not d["computing"] and d, "the suggestions")
    calls = (fake / "calls.log").read_text() if (fake / "calls.log").exists() else "never called"
    assert found["installers"] == ["photo-tool_3.2.0_amd64.deb"], calls

    _fake_dpkg_query(fake, "3.1.0")  # an older version is installed
    app.action("win.reload")
    app.wait_for(lambda: (d := app.state()["downloads"]) and not d["computing"] and d["installers"] == [],
                 "the installer to no longer be suggested", timeout=15)


def test_E4_07_grouping_moves_nothing(app, home):
    app.open_folder(home / "Downloads")
    before = home.snapshot("Downloads")
    app.action("win.group-by", s("kind"))
    app.wait_for(lambda: app.folder()["group_by"] == "kind", "grouping by kind")
    app.action("win.group-by", s("none"))
    app.wait_for(lambda: app.folder()["group_by"] == "none", "no grouping")
    app.wait_idle()
    assert home.snapshot("Downloads") == before


def test_E4_08_another_folder_as_downloads(app, home):
    chosen = home / "Documents" / "Invoices"
    app.action("win.set-downloads", s(str(chosen)))
    app.wait_for(lambda: app.state()["downloads"]["folder"] == str(chosen), "the preference")
    entries = {e["key"]: e for e in app.state()["sidebar"]["entries"]}
    app.wait_for(lambda: {e["key"]: e for e in app.state()["sidebar"]["entries"]}["place:downloads"]["path"]
                 == str(chosen), "the sidebar's Downloads entry")
    del entries
    app.action("win.tidy-downloads")
    app.wait_for(lambda: (d := app.state()["active_window"]["dialog"]) and d["type"] == "tidy", "the Tidy plan")
    plan = app.state()["tidy"]
    assert plan["moves"] and all(Path(m["from"]).parent == chosen for m in plan["moves"])
    assert app.folder()["path"] == str(chosen)
