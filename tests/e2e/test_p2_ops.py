"""Phase 2 end-to-end scenarios: file operations and Undo (DESIGN.md §10, Phase 2).

Every scenario checks the real (fixture) filesystem, not only what the app says.
"""

import os
import signal
import stat
import subprocess
import sys
from pathlib import Path

import pytest
from gi.repository import GLib

pytestmark = pytest.mark.p2

REPO = Path(__file__).resolve().parents[2]


def s(text: str) -> GLib.Variant:
    return GLib.Variant("s", text)


def ops(app) -> dict:
    return app.state()["ops"]


def trash_files(home) -> Path:
    return home / ".local" / "share" / "Trash" / "files"


def in_trash(home) -> list[str]:
    folder = trash_files(home)
    return sorted(p.name for p in folder.iterdir()) if folder.exists() else []


def toast(app) -> str:
    return app.state()["last_toast"] or ""


def wait_toast(app, part: str) -> str:
    return app.wait_for(lambda: part in toast(app) and toast(app), f"a toast with {part!r}")


def journal(app) -> list[dict]:
    return ops(app)["journal"]


# --- E2-01 ------------------------------------------------------------------------------

def test_E2_01_new_folder_and_rename_then_undo_both(app, home):
    app.open_folder(home / "Downloads")
    before = home.snapshot("Downloads")

    app.press("ctrl+shift+n")
    app.wait_for(lambda: (home / "Downloads" / "untitled folder").is_dir(), "the new folder")
    # The new folder opens for renaming; Escape keeps the name.
    app.wait_for(lambda: ops(app)["renaming"], "the new folder's name to be editable")
    app.press("escape")
    app.wait_for(lambda: not ops(app)["renaming"], "the editor to close")

    app.select("notes.txt")
    app.press("f2")
    app.wait_for(lambda: ops(app)["renaming"], "the rename editor")
    app.type_text("todo")  # the name before ".txt" starts selected
    app.press("return")
    app.wait_for(lambda: (home / "Downloads" / "todo.txt").exists(), "the file to be renamed")
    assert not (home / "Downloads" / "notes.txt").exists()
    app.wait_idle()
    assert "no app could open" not in (app.state()["last_toast"] or "").lower(), "Return only renamed"

    app.press("ctrl+z")
    app.wait_for(lambda: (home / "Downloads" / "notes.txt").exists(), "the rename to be undone")
    app.press("ctrl+z")
    app.wait_for(lambda: not (home / "Downloads" / "untitled folder").exists(), "the folder to go")
    app.wait_idle()
    assert home.snapshot("Downloads") == before


# --- E2-02 ------------------------------------------------------------------------------

def test_E2_02_trash_three_files_then_undo(app, home):
    names = ["notes.txt", "Budget-Q4.ods", "export.csv"]
    app.open_folder(home / "Downloads")
    before = home.snapshot("Downloads")

    app.select(*names)
    app.press("delete")
    app.wait_for(lambda: all(not (home / "Downloads" / n).exists() for n in names), "files to leave")
    assert sorted(names) == sorted(set(in_trash(home)) & set(names))
    wait_toast(app, "to the Trash")

    app.a11y_click("Undo", role="button")
    app.wait_for(lambda: all((home / "Downloads" / n).exists() for n in names), "files to come back")
    app.wait_idle()
    # Same paths, same contents, same modification times.
    assert home.snapshot("Downloads") == before
    assert not set(in_trash(home)) & set(names)


# --- E2-03 and E2-04 ----------------------------------------------------------------------

def test_E2_03_copy_paste_then_undo_removes_only_the_copies(app, home):
    names = ["notes.txt", "export.csv"]
    app.open_folder(home / "Downloads")
    downloads = home.snapshot("Downloads")
    documents = home.snapshot("Documents")
    app.select(*names)
    app.press("ctrl+c")
    app.open_folder(home / "Documents")
    app.press("ctrl+v")
    app.wait_idle()
    for n in names:
        assert (home / "Documents" / n).read_bytes() == (home / "Downloads" / n).read_bytes()
    # The copies reach the folder through its file monitor, which is slower
    # on older GLib (24.04): wait for the selection rather than read it once.
    app.wait_for(lambda: sorted(app.folder()["selection"]) == sorted(names), "the copies to be selected")

    app.press("ctrl+z")
    app.wait_for(lambda: not any((home / "Documents" / n).exists() for n in names), "copies to go")
    app.wait_idle()
    assert home.snapshot("Documents") == documents
    assert home.snapshot("Downloads") == downloads, "the originals are untouched"


def test_E2_04_cut_paste_then_undo_moves_back(app, home):
    names = ["notes.txt", "Receipts"]
    app.open_folder(home / "Downloads")
    before = (home.snapshot("Downloads"), home.snapshot("Documents"))
    app.select(*names)
    app.press("ctrl+x")
    app.open_folder(home / "Documents")
    app.press("ctrl+v")
    app.wait_idle()
    assert all((home / "Documents" / n).exists() for n in names)
    assert not any((home / "Downloads" / n).exists() for n in names)

    app.press("ctrl+z")
    app.wait_idle()
    assert (home.snapshot("Downloads"), home.snapshot("Documents")) == before


# --- E2-05 ------------------------------------------------------------------------------

def _paste_into_documents(app, home):
    app.open_folder(home / "Downloads")
    app.select("notes.txt")
    app.press("ctrl+c")
    app.open_folder(home / "Documents")
    app.press("ctrl+v")
    app.wait_for(lambda: (d := app.state()["active_window"]["dialog"]) and d["type"] == "clash",
                 "the clash dialog")


def test_E2_05_clash_skip_keep_both_replace(app, home):
    existing = home / "Documents" / "notes.txt"
    existing.write_text("the old notes\n")
    new = (home / "Downloads" / "notes.txt").read_bytes()

    _paste_into_documents(app, home)
    assert ops(app)["clash"]["name"] == "notes.txt"
    before = home.snapshot("Documents")
    app.a11y_click("Skip", role="button")
    app.wait_idle()
    assert home.snapshot("Documents") == before, "Skip changes nothing"

    _paste_into_documents(app, home)
    app.a11y_click("Keep Both", role="button")
    app.wait_idle()
    assert (home / "Documents" / "notes (2).txt").read_bytes() == new
    assert existing.read_text() == "the old notes\n"

    _paste_into_documents(app, home)
    app.a11y_click("Replace", role="button")
    app.wait_idle()
    assert existing.read_bytes() == new
    assert "notes.txt" in in_trash(home), "the replaced file went to the Trash"

    app.press("ctrl+z")
    app.wait_idle()
    assert existing.read_text() == "the old notes\n", "Undo brought the old file back"


# --- E2-06 ------------------------------------------------------------------------------

def test_E2_06_undo_never_overwrites(app, home):
    app.open_folder(home / "Downloads")
    app.select("notes.txt")
    app.press("ctrl+x")
    # Key presses travel through the compositor: wait for their effect, not
    # just an idle queue (under load the paste may not have started yet).
    app.wait_for(lambda: (app.state()["last_toast"] or "").startswith("Cut"), "the cut")
    app.open_folder(home / "Documents")
    app.press("ctrl+v")
    app.wait_for(lambda: (home / "Documents" / "notes.txt").exists()
                 and not (home / "Downloads" / "notes.txt").exists(), "the paste")
    app.wait_idle()
    old_place = home / "Downloads" / "notes.txt"
    old_place.write_text("a new file with the old name\n")

    app.press("ctrl+z")
    app.wait_idle()
    text = wait_toast(app, "Could not put back")
    assert "notes.txt" in text
    assert old_place.read_text() == "a new file with the old name\n", "nothing overwritten"
    assert (home / "Documents" / "notes.txt").exists(), "left where it was"
    assert journal(app)[-1]["state"] == "partial"


# --- E2-07 ------------------------------------------------------------------------------

def test_E2_07_cancel_a_big_copy(session, home, app_factory):
    big = home / "Downloads" / "disk-image.raw"
    with open(big, "wb") as handle:
        handle.truncate(2 * 1024**3)  # 2 GB, sparse
    # A small pause per progress step keeps the copy running long enough.
    app = app_factory(extra_env={"STEWARD_OPS_STEP_DELAY_MS": "2"})
    app.open_folder(home / "Downloads")
    app.select("disk-image.raw")
    app.press("ctrl+c")
    app.open_folder(home / "Documents")
    before = home.snapshot("Documents")
    app.press("ctrl+v")

    job = app.wait_for(lambda: (j := ops(app)["jobs"]) and j[0]["bytes"] > 0 and j[0], "progress")
    assert job["totalBytes"] == 2 * 1024**3
    app.a11y_click("Operations", role="button")
    app.a11y_click(f"Cancel {job['summary']}", role="button")
    app.wait_for(lambda: not ops(app)["jobs"], "the job to stop", timeout=20)
    assert not (home / "Documents" / "disk-image.raw").exists(), "no partial file"
    assert home.snapshot("Documents") == before
    assert all(b["kind"] != "copy" for b in journal(app)), "nothing journaled"
    assert ops(app)["last_report"]["cancelled"]


# --- E2-08 ------------------------------------------------------------------------------

def _wl(session, *args, input_text=None):
    return subprocess.run(list(args), env=session.base_env(), input=input_text,
                          capture_output=True, text=True, timeout=10)


def test_E2_08_clipboard_formats(session, app, home):
    """[verify in headless] Skipped if the compositor offers no clipboard
    to clients without focus."""
    app.open_folder(home / "Downloads")
    app.select("notes.txt", "export.csv")
    app.press("ctrl+c")
    app.wait_idle()
    types = _wl(session, "wl-paste", "--list-types")
    if types.returncode != 0:
        pytest.skip(f"no clipboard access for other clients here: {types.stderr.strip()}")
    offered = types.stdout.split()
    assert "x-special/gnome-copied-files" in offered and "text/uri-list" in offered
    gnome = _wl(session, "wl-paste", "--type", "x-special/gnome-copied-files").stdout
    uris = [(home / "Downloads" / n).as_uri() for n in ("notes.txt", "export.csv")]
    assert gnome.split("\n")[0] == "copy" and sorted(gnome.split("\n")[1:3]) == sorted(uris)

    # A cut made by another app (as Files does it) is pasted as a move.
    # wl-copy keeps serving the clipboard until something else takes it.
    target = home / "Downloads" / "Budget-Q4.ods"
    entry = f"cut\n{target.as_uri()}"
    other = subprocess.Popen(["wl-copy", "--foreground", "--type", "x-special/gnome-copied-files"],
                             stdin=subprocess.PIPE, env=session.base_env(), text=True,
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    other.stdin.write(entry)
    other.stdin.close()
    try:
        app.wait_for(lambda: _wl(session, "wl-paste", "--type", "x-special/gnome-copied-files")
                     .stdout.startswith("cut"), "another app's cut on the clipboard")
        app.open_folder(home / "Documents")
        app.action("win.paste")
        app.wait_idle()
        assert (home / "Documents" / "Budget-Q4.ods").exists() and not target.exists()
    finally:
        other.kill()


# --- E2-09 ------------------------------------------------------------------------------

def test_E2_09_trash_view_restore_and_empty(app, home):
    app.open_folder(home / "Downloads")
    app.select("notes.txt", "export.csv")
    app.press("delete")
    app.wait_idle()
    app.a11y_click("Trash", within="Sidebar")
    app.wait_for(lambda: app.folder()["title"] == "Trash", "the Trash view")
    app.wait_for(lambda: len(app.folder()["items"]) == 2, "both items in the Trash")

    # Put one back.
    name_in_trash = next(n for n in in_trash(home) if n.startswith("notes"))
    app.select(name_in_trash)
    app.a11y_click("Put Back", role="button")
    app.wait_for(lambda: (home / "Downloads" / "notes.txt").exists(), "notes.txt to come back")

    # Empty: cancel first, then confirm.
    app.a11y_click("Empty Trash…", role="button")
    app.wait_for(lambda: (d := app.state()["active_window"]["dialog"]) and d["type"] == "empty-trash",
                 "the confirming dialog")
    app.a11y_click("Cancel", role="button", within="Empty the Trash?")
    app.wait_for(lambda: app.state()["active_window"]["dialog"] is None, "the dialog to close")
    assert len(in_trash(home)) == 1, "Cancel keeps everything"

    app.a11y_click("Empty Trash…", role="button")
    app.wait_for(lambda: app.state()["active_window"]["dialog"], "the confirming dialog")
    app.a11y_click("Empty Trash", role="button", within="Empty the Trash?")
    app.wait_for(lambda: not in_trash(home), "the Trash to be empty")
    app.wait_idle()
    assert not (home / "Downloads" / "export.csv").exists()
    # The emptied item is gone for good: Undo now reverts the earlier
    # "Put back" instead (nothing can bring export.csv back).
    assert all(not b["undoable"] for b in journal(app) if b["kind"] == "trash")


# --- E2-10 ------------------------------------------------------------------------------

def test_E2_10_paste_into_a_read_only_folder(app, home):
    locked = home / "Documents" / "Locked"
    locked.mkdir()
    locked.chmod(0o555)
    try:
        app.open_folder(home / "Downloads")
        app.select("notes.txt")
        app.press("ctrl+c")
        app.open_folder(locked)
        before = (home.snapshot("Downloads"), home.snapshot("Documents"))
        app.press("ctrl+v")
        app.wait_idle()
        assert "permission" in wait_toast(app, "Could not")
        assert (home.snapshot("Downloads"), home.snapshot("Documents")) == before, "nothing changed"
    finally:
        locked.chmod(0o755)


# --- E2-11 and E2-12 ------------------------------------------------------------------------

def test_E2_11_kill_during_a_big_move(session, home, app_factory):
    source = home / "Documents" / "Many"
    source.mkdir()
    names = [f"file-{i:03}.txt" for i in range(200)]
    for name in names:
        (source / name).write_text(name)
    dest = home / "Documents" / "Moved"
    dest.mkdir()

    app = app_factory(extra_env={"STEWARD_OPS_STEP_DELAY_MS": "25"})
    app.open_folder(source)
    app.wait_for(lambda: len(app.folder()["items"]) == 200, "200 files listed")
    app.action("win.select-all")
    app.press("ctrl+x")
    app.open_folder(dest)
    app.press("ctrl+v")
    app.wait_for(lambda: len(list(dest.iterdir())) >= 20, "the move to be under way", timeout=20)
    app.process.send_signal(signal.SIGKILL)
    app.process.wait(timeout=5)

    moved = {p.name for p in dest.iterdir()}
    left = {p.name for p in source.iterdir()}
    assert 0 < len(moved) < 200, "killed midway"
    assert moved | left == set(names) and not moved & left, "none lost or duplicated"

    again = app_factory()
    assert again.state()["ops"]["interrupted"], "the interrupted batch is reported"
    assert "interrupted" in (again.state()["last_toast"] or "")
    assert any(b["state"] == "partial" for b in journal(again))


def test_E2_12_undo_after_a_restart(session, home, app_factory):
    app = app_factory()
    app.open_folder(home / "Downloads")
    app.select("notes.txt")
    app.press("ctrl+x")
    app.open_folder(home / "Documents")
    app.press("ctrl+v")
    app.wait_idle()
    app.stop()

    again = app_factory()
    again.press("ctrl+z")
    again.wait_for(lambda: (home / "Downloads" / "notes.txt").exists(), "the move to be undone")
    assert not (home / "Documents" / "notes.txt").exists()


# --- E2-13 and E2-14 ------------------------------------------------------------------------

def test_E2_13_only_ops_changes_files(tmp_path):
    check = REPO / "tests" / "tools" / "check_ops.py"
    result = subprocess.run([sys.executable, "-I", str(check)], capture_output=True, text=True)
    assert result.returncode == 0, result.stdout + result.stderr
    # The check really finds a violation.
    bad = tmp_path / "bad.rs"
    bad.write_text("fn f() { std::fs::remove_file(\"x\").unwrap(); }\n")
    found = subprocess.run([sys.executable, "-I", str(check), str(bad)], capture_output=True, text=True)
    assert found.returncode == 1 and "remove_file" in found.stdout


def test_E2_14_trashing_a_link_keeps_its_target(app, home):
    target = home / "Documents"
    link = home / "Downloads" / "Docs link"
    os.symlink(target, link)
    before = home.snapshot("Documents")
    app.open_folder(home / "Downloads")
    app.select("Docs link")
    app.press("delete")
    app.wait_idle()
    assert not os.path.lexists(link), "the link went"
    assert stat.S_ISLNK(os.lstat(trash_files(home) / "Docs link").st_mode), "the link is in the Trash"
    assert home.snapshot("Documents") == before, "the target folder is untouched"
