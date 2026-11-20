"""Phase 3 end-to-end scenarios: index, folder sizes, search and Peek
(DESIGN.md §10, Phase 3). E3-10 (one million entries) is an L2 test:
crates/core/tests/index.rs, reported in artifacts/perf/E3-10.json."""

import os
import signal
from pathlib import Path

import pytest
from gi.repository import GLib

pytestmark = pytest.mark.p3

# What the indexer never covers (cairn_core::index::Rules::for_home).
SKIPPED = (".cache/cairn", ".cache/thumbnails", ".local/share/cairn", ".local/share/Trash")


def s(text: str) -> GLib.Variant:
    return GLib.Variant("s", text)


def visible_names_containing(home, part: str) -> set[str]:
    """Names on disk that search should find: not hidden, not skipped."""
    found = set()
    for root, dirs, files in os.walk(home.path):
        rel = Path(root).relative_to(home.path)
        dirs[:] = [d for d in dirs if not d.startswith(".")]
        if any(p.startswith(".") for p in rel.parts):
            continue
        for name in dirs + files:
            if not name.startswith(".") and part.lower() in name.lower():
                found.add(name)
    return found


def expected_files(home) -> int:
    """The manifest's files, plus the settings database (dconf's user file)
    that the session writes into the home as soon as a setting changes."""
    return home.manifest["totals"]["files"] + int((home / ".config" / "dconf" / "user").exists())


def search(app, query: str) -> dict:
    app.press("ctrl+k")
    app.wait_for(lambda: app.state()["search"]["open"], "the search overlay")
    app.wait_idle()
    app.type_text(query)
    return app.wait_for(lambda: (r := app.state()["search"]) and r["ran"] == query and r,
                        f"results for {query!r}")


# --- E3-01 and E3-02 ------------------------------------------------------------------

def test_E3_01_every_file_is_indexed(app, home):
    status = app.wait_index_idle()
    assert status["complete"]
    assert status["files"] == expected_files(home)


def test_E3_02_folder_cards_show_size_and_count(app, home):
    app.wait_index_idle()
    app.open_folder(home / "Documents")
    names = ["Invoices", "Manuals", "Clients", "Projects"]
    stats = app.wait_for(lambda: (f := app.state()["folder_stats"]) and all(n in f for n in names) and f,
                         "the folder numbers")
    for name in names:
        expected = home.manifest["folders"][f"Documents/{name}"]
        assert stats[name] == {"items": expected["items"], "bytes": expected["tree_bytes"]}, name
    cards = {c["name"]: c for c in app.folder()["cards"]}
    for name in names:
        items = home.manifest["folders"][f"Documents/{name}"]["items"]
        assert cards[name]["count"] == f"{items} item{'' if items == 1 else 's'}"


# --- E3-03 and E3-04 ------------------------------------------------------------------

def test_E3_03_find_anything(app, home):
    app.wait_index_idle()
    result = search(app, "invo")
    names = [r["name"] for r in result["results"]]
    assert set(names) == visible_names_containing(home, "invo")
    dirs = [r["dir"] for r in result["results"]]
    assert dirs == sorted(dirs, reverse=True), "folders are listed before files"
    chips = {c["id"]: c["count"] for c in result["chips"]}
    assert chips["all"] == len(names)
    for kind, count in chips.items():
        if kind != "all":
            assert count == sum(1 for r in result["results"] if r["kind"] == kind), kind


def _install_handler(home, mime: str) -> Path:
    calls = home / "handler-calls.txt"
    apps = home / ".local" / "share" / "applications"
    apps.mkdir(parents=True, exist_ok=True)
    (apps / "cairn-test-handler.desktop").write_text(
        "[Desktop Entry]\nType=Application\nName=Test Handler\nNoDisplay=true\n"
        f"MimeType={mime};\n"
        f"Exec=/bin/sh -c 'printf \"%%s\\\\n\" \"$@\" >> {calls}' sh %U\n")
    (home / ".config" / "mimeapps.list").write_text(
        f"[Default Applications]\n{mime}=cairn-test-handler.desktop\n")
    return calls


def _select_first_file(app):
    def on_file():
        r = app.state()["search"]
        if r["results"] and not r["results"][r["selected"]]["dir"]:
            return r["results"][r["selected"]]
        app.press("down")
        return None
    return app.wait_for(on_file, "a file to be chosen")


def test_E3_04_open_and_show_in_folder(session, home, app_factory):
    calls = _install_handler(home, "application/pdf")
    app = app_factory()
    app.wait_index_idle()
    broad = search(app, "invo")["results"]
    app.type_text("ice-0")  # keep typing: "invoice-0"
    narrow = app.wait_for(lambda: (r := app.state()["search"]) and r["ran"] == "invoice-0" and r,
                          "the narrower results")
    assert 0 < len(narrow["results"]) <= len(broad)
    assert all("invoice-0" in r["name"].lower() for r in narrow["results"])

    first = _select_first_file(app)
    app.press("return")
    app.wait_for(lambda: calls.exists() and first["path"] in calls.read_text()
                 or calls.exists() and Path(first["path"]).as_uri() in calls.read_text(),
                 "the handler to open the first file")
    assert not app.state()["search"]["open"]

    app.press("ctrl+k")
    app.wait_for(lambda: app.state()["search"]["open"], "the search overlay again")
    app.press("down")
    app.press("down")
    other = _select_first_file(app)
    app.press("ctrl+return")
    parent = str(Path(other["path"]).parent)
    app.wait_for(lambda: app.folder()["path"] == parent, "its folder")
    app.wait_for(lambda: app.folder()["selection"] == [other["name"]], "it to be selected")


# --- E3-05 to E3-07 -------------------------------------------------------------------

def test_E3_05_a_new_file_is_found_within_5_s(app, home):
    app.wait_index_idle()
    search(app, "zebra-e305")
    (home / "Documents" / "zebra-e305-notes.txt").write_text("hello\n")
    app.wait_for(lambda: [r["name"] for r in app.state()["search"]["results"]] == ["zebra-e305-notes.txt"],
                 "the new file in the results", timeout=5)


def test_E3_06_renaming_in_the_app(app, home):
    app.wait_index_idle()
    app.open_folder(home / "Downloads")
    app.select("notes.txt")
    app.press("f2")
    app.wait_for(lambda: app.state()["ops"]["renaming"], "the rename editor")
    app.type_text("quokka")
    app.press("return")
    app.wait_for(lambda: (home / "Downloads" / "quokka.txt").exists(), "the rename")
    app.wait_index_idle()
    assert [r["name"] for r in search(app, "quokka")["results"]] == ["quokka.txt"]
    app.press("escape")
    old = search(app, "notes.txt")["results"]
    assert all(r["path"] != str(home / "Downloads" / "notes.txt") for r in old)


def test_E3_07_a_deleted_index_rebuilds(session, home, app_factory):
    app = app_factory()
    app.wait_index_idle()
    pid = app.index_status()["pid"]
    app.stop()
    # The indexer goes with the app it was started by.
    def gone():
        try:
            os.kill(pid, 0)
            return False
        except ProcessLookupError:
            return True
    app.wait_for(gone, "the indexer to stop")
    for suffix in ("", "-wal", "-shm"):
        Path(f"{home / '.cache' / 'cairn' / 'index.db'}{suffix}").unlink(missing_ok=True)

    again = app_factory()
    status = again.wait_index_idle()
    assert status["files"] == expected_files(home)
    assert "Invoices" in [r["name"] for r in search(again, "Invoices")["results"]]


# --- E3-08 ----------------------------------------------------------------------------

def _peek(app, name: str) -> dict:
    app.select(name)
    app.press("space")
    return app.wait_for(lambda: (p := app.state()["peek"]) and p["open"] and p["name"] == name and p,
                        f"Peek on {name}")


def test_E3_08_peek(app, home):
    app.open_folder(home / "Downloads")
    image = _peek(app, "beach-sunset.jpg")
    assert image["mode"] == "image"
    app.press("right")
    app.wait_for(lambda: app.state()["peek"]["name"] != "beach-sunset.jpg", "the next file")
    app.press("escape")
    app.wait_for(lambda: not app.state()["peek"]["open"], "Peek to close")

    pdf = _peek(app, "Statement-0923.pdf")
    assert pdf["mode"] == "pdf"
    app.wait_for(lambda: (app.state()["peek"]["pages"] or 0) >= 1, "the page count")
    app.press("escape")

    text = _peek(app, "notes.txt")
    assert text["mode"] == "text" and text["has_text"]
    app.press("escape")

    video = _peek(app, "holiday-video.mp4")
    assert video["mode"] == "media"
    app.press("escape")
    app.wait_for(lambda: not app.state()["peek"]["open"], "Peek to close")


# --- E3-09 ----------------------------------------------------------------------------

def test_E3_09_a_folder_name_in_the_path_bar(app, home):
    app.wait_index_idle()
    app.press("ctrl+l")
    app.wait_for(lambda: app.state()["path_bar"]["editing"], "the path field")
    app.wait_idle()
    app.type_text("Invoices")
    matches = app.wait_for(lambda: (m := app.state()["path_bar"]["matches"]) and len(m) >= 2 and m,
                           "the folders called Invoices")
    places = {(m["name"], m["where"]) for m in matches if m["name"] == "Invoices"}
    assert places == {("Invoices", "~/Documents"), ("Invoices", "~/Documents/Clients/Northwind")}
    app.press("down")
    app.press("return")
    app.wait_for(lambda: app.folder()["path"] == str(home / "Documents" / "Invoices"), "the first one")


# --- E3-11 and E3-12 ------------------------------------------------------------------

def test_E3_11_pause_and_resume_in_preferences(app, home):
    app.wait_index_idle()
    app.action("app.preferences")
    app.wait_for(lambda: app.state()["active_window"]["dialog"], "Preferences")
    # Since P6-T03 the index settings have their own page.
    app.a11y_click("Search and Index", within="Preferences")
    app.wait_for(lambda: "Pause indexing" in app.a11y_names(), "the Search and Index page")
    app.a11y_click("Pause indexing", within="Preferences")
    app.wait_for(lambda: app.index_status()["paused"], "the indexer to pause")
    app.a11y_click("Pause indexing", within="Preferences")
    app.wait_for(lambda: not app.index_status()["paused"], "the indexer to resume")


def test_E3_12_browsing_without_the_indexer(app, home):
    app.wait_index_idle()
    os.kill(app.index_status()["pid"], signal.SIGKILL)
    app.wait_for(lambda: not app.state()["index"]["running"], "the app to notice")

    app.open_folder(home / "Documents")
    assert len(app.folder()["items"]) > 0, "browsing still works"
    app.wait_for(lambda: any(c["count"] for c in app.folder()["cards"]), "live item counts")
    assert app.state()["folder_stats"] == {}, "no sizes without the index"

    app.press("ctrl+k")
    app.wait_for(lambda: app.state()["search"]["open"], "the search overlay")
    assert not app.state()["search"]["running"]
    app.a11y_click("Start Indexing", role="button")
    app.wait_index_idle()
    app.wait_for(lambda: app.state()["search"]["running"], "search to work again")
