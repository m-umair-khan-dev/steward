"""Screenshots for the website and the AppStream metadata (P6-T11).

Marked `shots`: `make shots` runs only these and writes light and dark
captures to artifacts/shots/<name>-<theme>.png. The normal gate skips them;
they test nothing beyond the screens opening."""

import os
from pathlib import Path

import pytest
from gi.repository import GLib

pytestmark = pytest.mark.shots

REPO = Path(__file__).resolve().parents[2]
OUT = Path(os.environ.get("SHOTS_DIR", REPO / "artifacts" / "shots"))
FIXTURE = REPO / "tests" / "data" / "sysinfo" / "fixture.json"


def s(text: str) -> GLib.Variant:
    return GLib.Variant("s", text)


def screens(app, home):
    """(name, how to get there) in the order the site shows them."""
    def folder():
        app.open_folder(home / "Downloads")

    def tidy():
        app.open_folder(home / "Downloads")
        app.action("win.tidy")
        app.wait_for(lambda: app.state()["active_window"]["dialog"], "the Tidy plan")

    def clean():
        app.action("win.tool-open", s("clean"))
        app.wait_for(lambda: (c := app.state()["clean"]) and c["cards"] and not c["looking"], "Free Up Space", 60)

    def workspace():
        app.action("win.workspace-create", s("Taxes 2026"))
        app.wait_for(lambda: app.state()["workspace"]["open"], "a workspace")
        wid = app.state()["workspace"]["id"]
        for rel in ("Documents/Invoices", "Documents/Clients", "Documents/Taxes 2026", "Downloads/beach-sunset.jpg"):
            app.action("win.workspace-add", s(f"{wid}:{home / rel}"))
        app.wait_for(lambda: len(app.state()["workspace"]["items"] or []) == 4, "the items")

    def dual():
        app.open_folder(home / "Downloads")
        app.press("f3")
        app.action("win.pane-go", s(str(home / "Documents" / "Invoices")))
        app.wait_for(lambda: app.state()["pane"]["items"], "the right pane")

    def search():
        app.open_folder(home.path)
        app.press("ctrl+k")
        app.wait_for(lambda: app.state()["search"]["open"], "search")
        app.type_text("invo")
        app.wait_for(lambda: app.state()["search"]["ran"] == "invo", "results")

    def installed():
        app.action("win.tool-open", s("installed"))
        app.wait_for(lambda: (i := app.state()["installed"]) and i["loaded"] and not i["looking"], "Installed", 60)

    def about():
        app.action("win.tool-open", s("about"))
        app.wait_for(lambda: app.state()["about"]["loaded"], "About")

    return [("folder", folder), ("tidy", tidy), ("free-up-space", clean), ("workspace", workspace),
            ("dual-pane", dual), ("search", search), ("installed", installed), ("about", about)]


@pytest.mark.parametrize("theme", ["light", "dark"])
def test_shots(app_factory, home, theme):
    OUT.mkdir(parents=True, exist_ok=True)
    app = app_factory(extra_env={"CAIRN_MOTION": "1", "CAIRN_FAKE_NOW": home.manifest["now"],
                                 "CAIRN_SYSINFO_FIXTURE": str(FIXTURE)})
    app.action("app.color-scheme", s(theme))
    app.wait_for(lambda: app.state()["dark"] == (theme == "dark"), f"the {theme} style")
    for name, go in screens(app, home):
        go()
        app.wait_idle()
        app.screenshot(OUT / f"{name}-{theme}.png")
        # Back to a plain window for the next screen.
        app.press("escape")
        app.action("win.tool-close")
        app.action("win.workspace-close")
        if app.state()["pane"]["open"]:
            app.press("f3")
        if app.state()["active_window"]["dialog"]:
            app.action("win.tidy-cancel")
