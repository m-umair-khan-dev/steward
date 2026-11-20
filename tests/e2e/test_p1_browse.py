"""Phase 1 end-to-end scenarios: Browse (DESIGN.md §10, Phase 1)."""

import os
import stat
import time

import pytest
from gi.repository import GLib
from PIL import Image, ImageChops, ImageStat

pytestmark = pytest.mark.p1

KIND_LABELS = {  # fixture generator label -> app section label
    "Images": "Images", "Videos": "Videos", "PDFs": "PDFs", "Documents": "Documents",
    "Archives": "Archives", "Installers": "Installers", "Code": "Code", "Other": "Other",
}
KIND_ORDER = ["Folders", "Images", "Videos", "Audio", "PDFs", "Documents", "Archives",
              "Installers", "Code", "Other"]


def s(text: str) -> GLib.Variant:
    return GLib.Variant("s", text)


def test_E1_01_open_downloads(app, home):
    folder = app.open_folder(home / "Downloads")
    facts = home.manifest["folders"]["Downloads"]
    assert folder["state"] == "ready"
    assert len(folder["items"]) == facts["items"]
    assert folder["status"].startswith(f"{facts['items']} items")
    assert folder["summary"].startswith(f"{facts['dirs']} folders, {facts['files']} files")
    app.wait_for(lambda: app.folder()["grid_headers"][:2] == [f"Folders {facts['dirs']}",
                                                              f"Files {facts['files']}"],
                 "the Folders and Files sections")


def test_E1_02_view_size_and_restart(app, app_factory, home):
    app.action("win.view-mode", s("list"))
    app.action("win.zoom-in")
    app.wait_for(lambda: app.folder()["view_mode"] == "list" and app.folder()["page"] == "list",
                 "list view")
    app.wait_for(lambda: app.folder()["row_size"] == "large", "large rows")

    app.stop()
    again = app_factory()
    folder = again.folder()
    assert folder["path"] == str(home.path)
    assert folder["view_mode"] == "list"
    assert folder["row_size"] == "large"


def test_E1_03_enter_back_forward_up(app, home):
    app.action("win.toggle-preview")
    app.open_folder(home.path)
    app.select("Documents")
    preview = app.wait_for(lambda: app.state()["preview"] if app.state()["preview"]["name"] == "Documents"
                           else None, "the preview of Documents")
    assert preview["visible"]
    app.wait_for(lambda: "Invoices" in app.state()["preview"]["children"], "Documents' children")

    app.action("win.open-selected")
    app.wait_for(lambda: app.folder()["path"] == str(home / "Documents"), "Documents to open")
    assert app.state()["path_bar"]["segments"][-1] == "Documents"
    assert app.state()["navigation"]["can_go_back"]

    app.action("win.back")
    app.wait_for(lambda: app.folder()["path"] == str(home.path), "back to home")
    assert app.state()["navigation"]["can_go_forward"]
    app.action("win.forward")
    app.wait_for(lambda: app.folder()["path"] == str(home / "Documents"), "forward to Documents")
    app.action("win.up")
    app.wait_for(lambda: app.folder()["path"] == str(home.path), "up to home")

    app.open_folder(home / "Downloads")
    app.select("Statement-0923.pdf")
    facts = dict(app.wait_for(lambda: app.state()["preview"]["facts"]
                              if app.state()["preview"]["name"] == "Statement-0923.pdf" else None,
                              "the preview of a PDF"))
    assert facts["Kind"].startswith("PDF")
    assert facts["Where"] == "~/Downloads"


def test_E1_04_path_bar_with_real_keys(app, home):
    app.press("ctrl+l")
    app.wait_for(lambda: app.state()["path_bar"]["editing"], "the path field")
    app.type_text("~/Doc")
    app.press("tab")
    app.wait_for(lambda: app.state()["path_bar"]["text"] == "~/Documents/", "Tab completion")
    app.press("return")
    app.wait_for(lambda: app.folder()["path"] == str(home / "Documents"), "Documents to open")
    assert not app.state()["path_bar"]["editing"]


def test_E1_05_group_by_kind(app, home):
    app.open_folder(home / "Downloads")
    app.action("win.group-by", s("kind"))
    manifest = home.manifest
    expected = {"Folders": manifest["folders"]["Downloads"]["dirs"]}
    for label, files in manifest["downloads"]["by_kind"].items():
        expected[KIND_LABELS[label]] = expected.get(KIND_LABELS[label], 0) + len(files)
    expected["Other"] = expected.get("Other", 0) + len(manifest["downloads"]["partial"])
    expected_sections = [(name, expected[name]) for name in KIND_ORDER if name in expected]

    folder = app.wait_for(lambda: app.folder() if app.folder()["group_by"] == "kind" else None,
                          "grouping by kind")
    assert [(s["label"], s["count"]) for s in folder["sections"]] == expected_sections

    # The grid shows the kinds as kind folders; opening one filters to it.
    layout = app.wait_for(lambda: app.folder()["grid_headers"] if any(
        h.startswith("By kind") for h in app.folder()["grid_headers"]) else None, "the By kind section")
    assert layout[0] == f"Folders {expected['Folders']}"
    assert layout[1] == f"By kind {len(expected_sections) - 1}"
    app.action("win.filter-kind", s("images"))
    app.wait_for(lambda: app.folder()["grid_headers"] == [f"Images {expected['Images']}"],
                 "the Images view")


def test_E1_06_hidden_files(app, home):
    app.open_folder(home.path)
    assert ".config" not in app.folder()["items"]
    app.action("win.show-hidden")
    app.wait_for(lambda: ".config" in app.folder()["items"], ".config to appear")
    app.action("win.show-hidden")
    app.wait_for(lambda: ".config" not in app.folder()["items"], ".config to disappear")


def test_E1_07_live_refresh(app, home):
    app.open_folder(home / "Desktop")
    new = home / "Desktop" / "arrived.txt"
    new.write_text("hello")
    app.wait_for(lambda: "arrived.txt" in app.folder()["items"], "the new file", timeout=2)
    new.unlink()
    app.wait_for(lambda: "arrived.txt" not in app.folder()["items"], "the file to go", timeout=2)


def test_E1_08_ten_thousand_files(app, home, record_property):
    big = home / "big"
    big.mkdir()
    for n in range(10_000):
        (big / f"file-{n:05d}.txt").touch()
    started = time.monotonic()
    folder = app.open_folder(big, timeout=60)
    app.wait_for(lambda: app.folder()["last_load_ms"] is not None, "the first frame", timeout=30)
    folder = app.folder()
    record_property("open_10k_seconds", round(time.monotonic() - started, 2))
    record_property("first_paint_ms", folder["last_load_ms"])
    assert len(folder["items"]) == 10_000
    # DESIGN.md: fewer than 300 row widgets. A grid row holds several cards.
    assert 0 < folder["bound_rows"] < 300, "only rows near the screen are built"
    assert folder["bound_cards"] < 1_000, "only on-screen items get cards"


def test_E1_09_thumbnails(app, home):
    folder = home / "Pictures" / "Holiday 2026"
    app.open_folder(folder)
    app.wait_for(lambda: (cards := app.folder()["cards"]) and all(c["thumbnail"] for c in cards),
                 "real previews on every card", timeout=30)
    cache = home / ".cache" / "thumbnails" / "large"
    assert len(list(cache.glob("*.png"))) >= 5


def test_E1_10_sidebar_open_hide_restart(app, app_factory, home):
    app.a11y_click("Documents", within="Sidebar")
    app.wait_for(lambda: app.folder()["path"] == str(home / "Documents"), "Documents from the sidebar")
    assert app.state()["sidebar"]["selected"] == "place:documents"

    app.action("win.sidebar-hide", s("place:music"))
    app.wait_for(lambda: "place:music" in app.state()["sidebar"]["hidden"], "Music to be hidden")
    assert all(e["key"] != "place:music" for e in app.state()["sidebar"]["entries"])

    app.stop()
    again = app_factory()
    assert "place:music" in again.state()["sidebar"]["hidden"]
    assert all(e["key"] != "place:music" for e in again.state()["sidebar"]["entries"])


def test_E1_11_permission_denied(app, home):
    locked = home / "locked"
    locked.mkdir()
    os.chmod(locked, 0)
    try:
        folder = app.open_folder(locked)
        assert folder["state"] == "permission-denied"
        app.wait_for(lambda: app.folder()["page"] == "status", "the explanation page")
        assert app.folder()["status_title"] == "You Can't Open This Folder"
        assert app.running()
    finally:
        os.chmod(locked, stat.S_IRWXU)


def test_E1_12_open_a_text_file(session, home, app_factory):
    calls = home / "handler-calls.txt"
    apps = home / ".local" / "share" / "applications"
    apps.mkdir(parents=True, exist_ok=True)
    (apps / "cairn-test-handler.desktop").write_text(
        "[Desktop Entry]\nType=Application\nName=Test Handler\nNoDisplay=true\n"
        "MimeType=text/plain;\n"
        f"Exec=/bin/sh -c 'printf \"%%s\\\\n\" \"$@\" >> {calls}' sh %U\n")
    (home / ".config" / "mimeapps.list").write_text(
        "[Default Applications]\ntext/plain=cairn-test-handler.desktop\n")

    app = app_factory()
    app.open_folder(home / "Downloads")
    app.select("notes.txt")
    app.action("win.open-selected")
    target = home / "Downloads" / "notes.txt"
    # GIO hands a local file to the app as a path or a file:// URI.
    app.wait_for(lambda: calls.exists() and (str(target) in calls.read_text()
                                             or target.as_uri() in calls.read_text()),
                 "the handler to be called")


def _mean_luminance(path) -> float:
    with Image.open(path) as image:
        grey = image.convert("L").resize((64, 40))
    return ImageStat.Stat(grey).mean[0]


def test_E1_13_light_and_dark(app, tmp_path):
    app.action("app.color-scheme", s("light"))
    app.wait_for(lambda: not app.state()["dark"], "light style")
    light = app.screenshot(tmp_path / "light.png")
    app.action("app.color-scheme", s("dark"))
    app.wait_for(lambda: app.state()["dark"], "dark style")
    dark = app.screenshot(tmp_path / "dark.png")
    assert _mean_luminance(light) > 170, "light screenshot is not light"
    assert _mean_luminance(dark) < 90, "dark screenshot still has light areas"


def test_E1_14_shortcuts_window(app):
    accels = app.state()["accels"]
    # Qt key-sequence syntax since ADR 0009.
    assert accels["win.location"] == ["Ctrl+L"]
    assert accels["win.show-hidden"] == ["Ctrl+H"]
    assert accels["win.back"][0] == "Alt+Left"
    assert all(accels[name] for name in accels), f"actions without keys: {accels}"

    app.action("win.show-help-overlay")
    titles = ["Type a path or a folder name", "Back", "Forward", "Up to the parent folder",
              "Show hidden files", "Larger", "Smaller", "Preferences", "Quit"]
    def listed():
        # The shortcuts window spreads them over pages; count every page.
        names = app.a11y_names(showing_only=False)
        missing = [t for t in titles if t not in names]
        return not missing

    app.wait_for(listed, "the shortcuts window to list every shortcut")


def test_E1_15_browsing_changes_nothing(app, home):
    before = home.snapshot(ignore_app_owned=True)
    app.open_folder(home / "Downloads")
    app.action("win.group-by", s("kind"))
    app.action("win.filter-kind", s("pdfs"))
    app.action("win.filter-kind", s("all"))
    app.action("win.group-by", s("created"))
    app.action("win.view-mode", s("list"))
    app.action("win.sort-by", s("size"))
    app.action("win.toggle-preview")
    app.select("Statement-0923.pdf")
    app.open_folder(home / "Pictures" / "Holiday 2026")
    app.action("win.view-mode", s("grid"))
    app.wait_for(lambda: all(c["thumbnail"] for c in app.folder()["cards"]), "thumbnails", timeout=30)
    app.action("win.show-hidden")
    app.open_folder(home.path)
    app.action("win.sidebar-hide", s("place:videos"))
    app.action("win.zoom-out")
    app.wait_idle()
    assert home.snapshot(ignore_app_owned=True) == before


def test_E1_16_filter_this_folder(app, home):
    """Added with ADR 0008: the toolbar's "Filter this folder" field."""
    app.open_folder(home / "Downloads")
    total = len(app.folder()["items"])
    app.press("ctrl+f")
    app.wait_idle()  # the field takes the focus before the letters arrive
    app.type_text("invoice")
    expected = sorted(n for n in home.manifest["downloads"]["loose_files"] if "invoice" in n.lower())
    app.wait_for(lambda: sorted(app.folder()["items"]) == expected, "only the matching items")
    app.wait_for(lambda: app.folder()["status"].startswith(f"{len(expected)} items"),
                 "the status line to count the matches")
    app.press("escape")
    app.wait_for(lambda: len(app.folder()["items"]) == total, "the whole folder again")
    assert app.folder()["name_filter"] == ""


def test_E1_17_sidebar_collapses_to_a_rail(app, home):
    """Added with ADR 0008: collapsed sidebar shows only icons. Expanding on
    pointer hover is on the HUMAN checklist (pointer input is not automated)."""
    app.action("win.toggle-sidebar")
    sidebar = app.wait_for(lambda: (s := app.state()["sidebar"]) and not s["expanded"] and s,
                           "the sidebar to collapse")
    assert sidebar["rail_visible"] and not sidebar["visible"]
    assert sidebar["rail"] == [e["label"] for e in sidebar["entries"]]
    app.a11y_click("Documents", within="Sidebar rail")
    app.wait_for(lambda: app.folder()["path"] == str(home / "Documents"), "Documents from the rail")

    app.action("win.toggle-sidebar")
    app.wait_for(lambda: app.state()["sidebar"]["expanded"] and app.state()["sidebar"]["visible"],
                 "the sidebar to expand again")
    assert not app.state()["sidebar"]["rail_visible"]


def _image_difference(a, b) -> float:
    """Mean absolute difference of two screenshots, 0..255."""
    with Image.open(a) as first, Image.open(b) as second:
        x = first.convert("L").resize((160, 100))
        y = second.convert("L").resize((160, 100))
    return ImageStat.Stat(ImageChops.difference(x, y)).mean[0]


def _by_kind_shown(app) -> bool:
    return any(h.startswith("By kind") for h in app.folder()["grid_headers"])


def test_E1_18_files_fly_into_kind_folders(session, home, app_factory, tmp_path):
    """Added with ADR 0009 (DESIGN.md §3.5): grouping by kind is animated,
    and immediate when motion is off."""
    app = app_factory(extra_env={"CAIRN_MOTION": "1"})
    assert app.state()["motion"], "CAIRN_MOTION=1 turns motion on in a test build"
    app.open_folder(home / "Downloads")
    app.wait_idle()
    before = app.screenshot(tmp_path / "before.png")

    app.action("win.group-by", s("kind"))
    app.wait_for(lambda: app.state()["flying"] > 0, "cards to take off", timeout=3)
    started = time.monotonic()
    middle = app.screenshot(tmp_path / "middle.png")
    still_flying = app.state()["flying"] > 0
    app.wait_for(lambda: app.state()["flying"] == 0, "cards to land", timeout=5)
    flight = time.monotonic() - started
    app.wait_for(lambda: _by_kind_shown(app), "the By kind section")
    app.wait_idle()
    after = app.screenshot(tmp_path / "after.png")

    # A movement over several frames, not a jump.
    assert still_flying and flight > 0.2, f"the flight lasted only {flight:.2f}s"
    assert _image_difference(middle, before) > 1.0, "the mid-flight frame equals the start"
    assert _image_difference(middle, after) > 1.0, "the mid-flight frame equals the end"
    app.stop()

    # Reduced motion: the same change happens at once, with nothing in flight.
    still = app_factory()
    assert not still.state()["motion"]
    still.open_folder(home / "Downloads")
    still.action("win.group-by", s("created"))
    still.action("win.group-by", s("kind"))
    seen = []
    still.wait_for(lambda: seen.append(still.state()["flying"]) or _by_kind_shown(still), "the By kind section")
    still.wait_idle()
    assert still.state()["flying"] == 0 and max(seen) == 0, f"cards flew with motion off: {seen}"
