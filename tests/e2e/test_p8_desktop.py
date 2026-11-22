"""Phase 8 end-to-end scenarios: around the desktop (E8-01 onwards).
The phone scenarios (E8-08, E8-09) are on hold by the product owner's decision."""

import os
import time

import pytest
from gi.repository import GLib

pytestmark = pytest.mark.p8


def s(text: str) -> GLib.Variant:
    return GLib.Variant("s", text)


def shelf(app) -> dict:
    return app.state()["shelf"]


def open_shelf(app) -> dict:
    app.action("app.shelf")
    return app.wait_for(lambda: (sh := shelf(app)) and sh["open"] and sh, "the Shelf")


def pin(app, *folders) -> None:
    for folder in folders:
        app.action("win.shelf-pin", s(str(folder)))
    app.wait_for(lambda: [p["path"] for p in shelf(app)["pins"]][-len(folders):] == [str(f) for f in folders],
                 "the pins")


# --- the Shelf (E8-01 to E8-04) -----------------------------------------------------

def test_E8_01_the_shelf_and_its_number_keys(app, home):
    pins = shelf(app)["pins"]
    assert len(pins) >= 2, "the Shelf offers the usual folders before any are chosen"
    state = open_shelf(app)
    for got in state["pins"]:
        expected = len([n for n in os.listdir(got["path"]) if not n.startswith(".")])
        assert got["count"] == expected, got
    names = app.a11y_names()
    second = pins[1]["path"]
    assert any(n.startswith("2: ") for n in names), "the second pin is numbered"

    app.press("2")
    app.wait_for(lambda: app.folder()["path"] == second, "the main window to open the second folder")
    app.wait_for(lambda: not shelf(app)["open"], "the Shelf to close")


def test_E8_02_drop_on_a_pinned_folder_then_undo(app, home):
    invoices = home / "Documents" / "Invoices"
    pin(app, invoices)
    n = len(shelf(app)["pins"])
    open_shelf(app)
    # A file whose name is free in Invoices (Invoice-0923.pdf is in both).
    source = home / "Downloads" / "Boarding-pass.pdf"
    before = source.read_bytes()

    app.action("win.shelf-drop", s(f"{n}|move|{source.as_uri()}"))
    result = app.wait_for(lambda: (r := shelf(app)["result"]) and r["undo"] and r, "the result line")
    assert "Boarding-pass.pdf" in result["text"] and "Invoices" in result["text"], result
    moved = invoices / "Boarding-pass.pdf"
    assert not source.exists() and moved.read_bytes() == before
    assert result["text"] in app.a11y_names()

    app.action("win.shelf-undo")
    app.wait_for(lambda: source.exists() and not moved.exists(), "Undo to move it back")
    app.wait_for(lambda: (r := shelf(app)["result"]) and not r["undo"], "the result line to say so")
    assert source.read_bytes() == before


def test_E8_03_holding_survives_a_restart_and_moves_nothing(app, app_factory, home):
    first = home / "Downloads" / "Invoice-0923.pdf"
    second = home / "Documents" / "Invoices"
    before = home.snapshot(ignore_app_owned=True)
    app.action("win.shelf-hold", s(f"{first}\n{second.as_uri()}"))
    app.wait_for(lambda: shelf(app)["held"] == [str(first), str(second)], "two items in Holding")

    app.stop()
    again = app_factory()
    assert shelf(again)["held"] == [str(first), str(second)], "Holding remembers them"
    open_shelf(again)
    assert {"Invoice-0923.pdf", "Invoices"} <= again.a11y_names()
    again.a11y_click("Clear", role="button", within="Holding")
    again.wait_for(lambda: shelf(again)["held"] == [], "Holding to be empty")
    assert first.exists() and second.is_dir()
    assert home.snapshot(ignore_app_owned=True) == before, "nothing on disk changed"


def test_E8_04_latest_downloads(app, home):
    downloads = home / "Downloads"
    now = time.time()
    planted = {
        "new-1.pdf": 10, "new-2.jpg": 20, "new-3.zip": 30, "new-4.txt": 40, "new-5.odt": 50, "new-6.png": 60,
        ".hidden-newest": 1, "half.iso.part": 2, "half.deb.crdownload": 3,
    }
    for name, age in planted.items():
        path = downloads / name
        path.write_bytes(b"x")
        os.utime(path, (now - age, now - age))
    folder = downloads / "brand-new-folder"
    folder.mkdir()
    os.utime(folder, (now, now))
    app.wait_for(lambda: shelf(app)["latest"] == ["new-1.pdf", "new-2.jpg", "new-3.zip", "new-4.txt", "new-5.odt"],
                 "the five newest files")
    open_shelf(app)
    assert "new-1.pdf" in app.a11y_names()
