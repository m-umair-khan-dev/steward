"""Captures review screenshots of the app against a fresh fixture home, for
comparison with the reference captures (P1-T16, HUMAN sign-off).

    make shell, then: docker/scripts/run-e2e python3 tests/tools/capture_screens.py

Writes artifacts/screens/*.png, including frames taken while files fly into
their kind folders.
"""

import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tests" / "e2e"))

from gi.repository import GLib  # noqa: E402

from harness import App, Home, Session, wait_for  # noqa: E402

OUT = REPO / "artifacts" / "screens"


def s(text: str) -> GLib.Variant:
    return GLib.Variant("s", text)


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        root = Path(tmp)
        source = root / "home"
        subprocess.run([sys.executable, "-I", str(REPO / "docker/scripts/make-fixture-home"), str(source)],
                       check=True, capture_output=True)
        home = Home(source)
        session = Session(root / "session", home.path)
        session.start()
        app = App(session, home, extra_env={"CAIRN_MOTION": "1"})
        try:
            app.launch()
            app.open_folder(home / "Downloads")
            app.action("win.toggle-preview")
            app.wait_idle()
            wait_for(lambda: True, "settle", 0.5)
            app.screenshot(OUT / "01-downloads-grid.png")

            app.action("win.group-by", s("kind"))
            wait_for(lambda: app.state()["flying"] > 0, "cards to take off", 3)
            for frame in range(4):
                app.screenshot(OUT / f"02-flight-{frame}.png")
            wait_for(lambda: app.state()["flying"] == 0, "cards to land", 5)
            app.wait_idle()
            app.screenshot(OUT / "03-by-kind.png")

            app.action("win.group-by", s("none"))
            app.select("Statement-0923.pdf")
            app.wait_idle()
            app.screenshot(OUT / "04-selected.png")

            app.action("win.view-mode", s("list"))
            app.wait_idle()
            app.screenshot(OUT / "05-list.png")

            app.action("win.view-mode", s("grid"))
            app.open_folder(home.path)
            app.action("app.color-scheme", s("dark"))
            wait_for(lambda: app.state()["dark"], "dark style", 5)
            app.wait_idle()
            app.screenshot(OUT / "06-home-dark.png")

            app.action("app.color-scheme", s("light"))
            app.action("win.toggle-sidebar")
            app.wait_idle()
            app.screenshot(OUT / "07-rail.png")
            app.action("win.toggle-sidebar")

            app.action("app.about")
            app.wait_idle()
            app.screenshot(OUT / "08-about.png")
            app.press("escape")
            app.wait_for(lambda: app.state()["active_window"]["dialog"] is None, "the dialog to close", 5)

            # Phase 2: rename in place, Undo toast, Trash view, clash dialog.
            app.open_folder(home / "Downloads")
            app.select("notes.txt")
            app.press("f2")
            app.wait_for(lambda: app.state()["ops"]["renaming"], "the rename editor", 5)
            app.wait_idle()
            app.screenshot(OUT / "09-rename.png")
            app.press("escape")

            app.select("export.csv", "Budget-Q4.ods")
            app.press("delete")
            app.wait_idle()
            app.screenshot(OUT / "10-trashed-toast.png")

            app.open_folder(home / ".local/share/Trash/files")
            app.wait_idle()
            app.screenshot(OUT / "11-trash-view.png")

            (home / "Documents" / "notes.txt").write_text("older notes\n")
            app.open_folder(home / "Downloads")
            app.select("notes.txt")
            app.press("ctrl+c")
            app.open_folder(home / "Documents")
            app.press("ctrl+v")
            wait_for(lambda: (d := app.state()["active_window"]["dialog"]) and d["type"] == "clash",
                     "the clash dialog", 5)
            app.wait_for(lambda: True, "settle", 0.3)
            app.screenshot(OUT / "12-clash.png")
            app.press("escape")

            # Phase 3: sizes, the outline, search, Peek, folder names.
            app.wait_index_idle()
            app.open_folder(home / "Documents")
            app.select("Clients")
            wait_for(lambda: app.state()["folder_stats"].get("Clients"), "folder sizes", 10)
            app.wait_idle()
            app.screenshot(OUT / "13-sizes-and-outline.png")

            app.press("ctrl+k")
            wait_for(lambda: app.state()["search"]["open"], "search", 5)
            app.wait_idle()
            app.type_text("invo")
            wait_for(lambda: app.state()["search"]["ran"] == "invo", "results", 5)
            app.wait_idle()
            app.screenshot(OUT / "14-search.png")
            app.press("escape")

            app.open_folder(home / "Downloads")
            app.select("Statement-0923.pdf")
            app.press("space")
            wait_for(lambda: (app.state()["peek"]["pages"] or 0) >= 1, "the PDF", 10)
            app.wait_idle()
            app.screenshot(OUT / "15-peek-pdf.png")
            app.press("escape")
            app.select("notes.txt")
            app.press("space")
            wait_for(lambda: app.state()["peek"]["open"], "Peek", 5)
            app.wait_idle()
            app.screenshot(OUT / "16-peek-text.png")
            app.press("escape")

            app.press("ctrl+l")
            wait_for(lambda: app.state()["path_bar"]["editing"], "the path field", 5)
            app.wait_idle()
            app.type_text("Invoices")
            wait_for(lambda: len(app.state()["path_bar"]["matches"]) >= 2, "folder names", 5)
            app.wait_idle()
            app.screenshot(OUT / "17-folder-names.png")
            app.press("escape")

            # Phase 4: Downloads panel, Tidy plan, dual pane, workspace, symbols.
            app.open_folder(home / "Downloads")
            wait_for(lambda: (d := app.state()["downloads"]) and d["installers"] is not None
                     and not d["computing"], "suggestions", 20)
            app.wait_idle()
            app.screenshot(OUT / "18-downloads-panel.png")
            app.press("ctrl+shift+o")
            wait_for(lambda: app.state()["active_window"]["dialog"], "the Tidy plan", 5)
            app.wait_idle()
            app.screenshot(OUT / "19-tidy-plan.png")
            app.action("win.tidy-cancel")

            app.press("f3")
            wait_for(lambda: app.state()["pane"]["open"], "two panes", 5)
            app.action("win.pane-go", GLib.Variant("s", str(home / "Documents" / "Invoices")))
            app.select("Statement-0923.pdf", "notes.txt")
            wait_for(lambda: app.state()["pane"]["items"], "the right pane", 5)
            app.wait_idle()
            app.screenshot(OUT / "20-dual-pane.png")
            app.press("f3")

            app.action("win.workspace-create", GLib.Variant("s", "Taxes 2026"))
            wait_for(lambda: app.state()["workspace"]["open"], "a workspace", 5)
            wid = app.state()["workspace"]["id"]
            for rel in ("Documents/Invoices", "Documents/Clients", "Documents/Taxes 2026", "Downloads/beach-sunset.jpg"):
                app.action("win.workspace-add", GLib.Variant("s", f"{wid}:{home / rel}"))
            app.action("win.workspace-open", GLib.Variant("s", str(wid)))
            wait_for(lambda: len(app.state()["workspace"]["items"] or []) == 4, "the items", 5)
            app.wait_idle()
            app.screenshot(OUT / "21-workspace.png")

            app.open_folder(home / "Documents")
            app.select("Taxes 2026")
            app.action("win.folder-style-ask")
            wait_for(lambda: app.state()["active_window"]["dialog"], "the chooser", 5)
            app.wait_idle()
            app.screenshot(OUT / "22-symbol-chooser.png")
            app.press("escape")
        finally:
            app.stop()
            session.stop()
    print("\n".join(sorted(str(p.relative_to(REPO)) for p in OUT.glob("*.png"))))


if __name__ == "__main__":
    main()
