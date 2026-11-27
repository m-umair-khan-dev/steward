"""Phase 11: Extract Here (GitHub issue #5), with the real pointer.

Archives unpack inside Steward, into a folder beside them, through the
operations engine: Undo trashes the folder, nothing escapes the folder,
and a broken archive says what is wrong instead of opening another app.
"""

import tarfile
import zipfile

from harness.wait import wait_for


def ops(app):
    return app.state()["ops"]


def make_zip(path, entries):
    with zipfile.ZipFile(path, "w") as archive:
        for name, text in entries.items():
            archive.writestr(name, text)


def extract_from_menu(app, name):
    app.select(name)
    wait_for(lambda: app.folder()["selection"] == [name], f"{name} selected")
    # The real right-click menu (the card is on screen: few items in Documents).
    app.mouse_click(name, role="button", button="right")
    app.mouse_click("Extract Here", role="menu item")


def test_E11_06_extract_here_and_undo(app, home):
    folder = home / "Documents"
    make_zip(folder / "photos.zip", {"a.txt": "one", "b/c.txt": "two"})
    app.open_folder(folder)
    wait_for(lambda: "photos.zip" in app.folder()["items"], "the archive listed")
    extract_from_menu(app, "photos.zip")
    out = folder / "photos"
    wait_for(lambda: (out / "b" / "c.txt").exists(), "the extracted folder")
    assert (out / "a.txt").read_text() == "one"
    assert (folder / "photos.zip").exists(), "the archive stays"
    assert not [p for p in folder.iterdir() if "steward-extract" in p.name], "no staging left"
    wait_for(lambda: "Extracted to “photos”" in (app.state()["last_toast"] or ""), "the toast")
    app.action("win.undo")
    wait_for(lambda: not out.exists(), "Undo to remove the folder")
    trash = home / ".local" / "share" / "Trash" / "files"
    assert "photos" in [p.name for p in trash.iterdir()], "Undo trashes, never deletes"


def test_E11_07_one_top_folder_is_not_nested_and_names_stay_free(app, home):
    folder = home / "Documents"
    (folder / "site").mkdir()  # the name is taken: the result gets a free one
    with tarfile.open(folder / "site-1.2.tar.gz", "w:gz") as archive:
        for name in ("site/index.html", "site/css/style.css"):
            path = home.path / "tmp-src" / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(name)
            archive.add(path, arcname=name)
    app.open_folder(folder)
    wait_for(lambda: "site-1.2.tar.gz" in app.folder()["items"], "the archive listed")
    app.select("site-1.2.tar.gz")
    app.action("win.extract-here")
    out = folder / "site (2)"
    wait_for(lambda: (out / "css" / "style.css").exists(), "site (2)/css/style.css")
    assert not (out / "site").exists(), "one top folder is used as is"


def test_E11_08_double_click_extracts(app, home):
    folder = home / "Documents"
    make_zip(folder / "notes-pack.zip", {"todo.txt": "x", "done.txt": "y"})
    app.open_folder(folder)
    wait_for(lambda: "notes-pack.zip" in app.folder()["items"], "the archive listed")
    app.select("notes-pack.zip")
    app.action("win.open-selected")
    wait_for(lambda: (folder / "notes-pack" / "todo.txt").exists(), "extracted by opening it")


def test_E11_09_nothing_escapes_the_folder(app, home):
    folder = home / "Documents"
    make_zip(folder / "evil.zip", {"../escaped.txt": "no", "ok.txt": "yes"})
    app.open_folder(folder)
    wait_for(lambda: "evil.zip" in app.folder()["items"], "the archive listed")
    app.select("evil.zip")
    app.action("win.extract-here")
    wait_for(lambda: not ops(app)["jobs"] and app.state()["last_toast"], "the extraction to end")
    assert not (home / "escaped.txt").exists() and not (folder.parent / "escaped.txt").exists()


def test_E11_10_a_broken_archive_says_so(app, home):
    folder = home / "Documents"
    (folder / "broken.zip").write_bytes(b"PK\x03\x04 this is not really a zip")
    app.open_folder(folder)
    wait_for(lambda: "broken.zip" in app.folder()["items"], "the archive listed")
    app.select("broken.zip")
    app.action("win.extract-here")
    toast = wait_for(lambda: (t := app.state()["last_toast"]) and "broken.zip" in t and t, "the error")
    assert "Could not" in toast
    assert not (folder / "broken").exists()
    assert not [p for p in folder.iterdir() if "steward-extract" in p.name], "no staging left"
