"""Phase 4 end-to-end scenarios: workspaces (DESIGN.md §10, Phase 4)."""

import os
import shutil
import uuid
from pathlib import Path

import pytest
from gi.repository import GLib

pytestmark = pytest.mark.p4


def s(text: str) -> GLib.Variant:
    return GLib.Variant("s", text)


def ws(app) -> dict:
    return app.state()["workspace"]


def create(app, name: str) -> int:
    app.action("win.workspace-create", s(name))
    page = app.wait_for(lambda: (w := ws(app)) and w["open"] and w["name"] == name and w, f"workspace {name}")
    return page["id"]


@pytest.fixture
def second_volume():
    """A folder on another filesystem: /dev/shm is its own tmpfs."""
    root = Path("/dev/shm") / f"steward-e2e-{uuid.uuid4().hex[:8]}"
    (root / "Footage").mkdir(parents=True)
    (root / "Footage" / "clip.mp4").write_bytes(b"\0" * 1024)
    yield root
    shutil.rmtree(root, ignore_errors=True)


def test_E4_11_a_workspace_from_three_places(app, home, second_volume):
    assert os.stat(second_volume).st_dev != os.stat(home.path).st_dev, "really another filesystem"
    before = (home.snapshot("Documents"), home.snapshot("Downloads"))
    wid = create(app, "Taxes 2026")

    app.open_folder(home / "Documents")
    app.select("Invoices")
    app.action("win.workspace-add", s(str(wid)))
    app.open_folder(home / "Downloads")
    app.select("notes.txt")
    app.action("win.workspace-add", s(str(wid)))
    app.open_folder(second_volume)
    app.select("Footage")
    app.action("win.workspace-add", s(str(wid)))

    app.action("win.workspace-open", s(str(wid)))
    items = app.wait_for(lambda: ws(app)["open"] and len(ws(app)["items"]) == 3 and ws(app)["items"], "three items")
    places = {i["name"]: i["where"] for i in items}
    assert places["Invoices"] == "~/Documents"
    assert places["notes.txt"] == "~/Downloads"
    assert places["Footage"] and not places["Footage"].startswith("~"), "labelled with its volume"
    assert all(i["state"] == "here" for i in items)
    app.wait_idle()
    assert (home.snapshot("Documents"), home.snapshot("Downloads")) == before, "nothing on disk moved"
    assert (second_volume / "Footage" / "clip.mp4").exists()


def test_E4_12_renamed_deleted_and_located(app, home):
    wid = create(app, "Manuals and code")
    app.action("win.workspace-add", s(f"{wid}:{home / 'Documents' / 'Manuals'}"))
    app.action("win.workspace-add", s(f"{wid}:{home / 'Documents' / 'Projects'}"))
    app.wait_for(lambda: len(ws(app)["items"]) == 2, "two items")

    os.rename(home / "Documents" / "Manuals", home / "Documents" / "Manuals 2026")
    shutil.rmtree(home / "Documents" / "Projects")
    app.action("win.workspace-tab", s("overview"))  # look again
    items = app.wait_for(lambda: (i := ws(app)["items"]) and {x["state"] for x in i} == {"here", "missing"} and i,
                         "one found again, one missing")
    found = next(i for i in items if i["state"] == "here")
    missing = next(i for i in items if i["state"] == "missing")
    assert found["name"] == "Manuals 2026", "found again by its inode"

    replacement = home / "Documents" / "Projects (new)"
    replacement.mkdir()
    app.action("win.workspace-locate", s(f"{missing['id']}:{replacement}"))
    app.wait_for(lambda: all(i["state"] == "here" for i in ws(app)["items"]), "Locate… to repair it")
    assert any(i["name"] == "Projects (new)" for i in ws(app)["items"])


def test_E4_13_alt_1_opens_the_first_workspace(app, home):
    create(app, "First")
    create(app, "Second")
    app.open_folder(home / "Downloads")
    app.wait_for(lambda: not ws(app)["open"], "the folder again")
    app.press("alt+1")
    app.wait_for(lambda: ws(app)["open"] and ws(app)["name"] == "First", "the first workspace")
