"""Screenshots for the website and the AppStream metadata (P6-T11).

Marked `shots`: `make shots` runs only these and writes light and dark
captures to artifacts/shots/<name>-<theme>.png. The normal gate skips them;
they test nothing beyond the screens opening."""

import os
import subprocess
from pathlib import Path

import pytest
from gi.repository import GLib
from harness import Home

pytestmark = pytest.mark.shots

REPO = Path(__file__).resolve().parents[2]
OUT = Path(os.environ.get("SHOTS_DIR", REPO / "artifacts" / "shots"))
FIXTURE = REPO / "tests" / "data" / "sysinfo" / "fixture.json"


# Where the screenshots' home lives: the path bar shows the whole path, so a
# home under /tmp/pytest-of-dev/... would show in every capture. The e2e
# service mounts an empty folder here (compose.desktop.yaml).
SHOTS_HOME = Path(os.environ.get("SHOTS_HOME", "/home/alex"))


@pytest.fixture
def home(fixture_source) -> Home:
    """The fixture home copied to SHOTS_HOME (planted times kept)."""
    source, times = fixture_source
    if not SHOTS_HOME.is_dir() or not os.access(SHOTS_HOME, os.W_OK):
        pytest.fail(f"{SHOTS_HOME} is not a writable folder; run through `make shots`")
    subprocess.run(["find", str(SHOTS_HOME), "-mindepth", "1", "-delete"], check=True)
    subprocess.run(["cp", "-a", "--sparse=always", f"{source}/.", str(SHOTS_HOME)], check=True)
    for rel, stamps in times.items():
        os.utime(SHOTS_HOME / rel, ns=stamps, follow_symlinks=False)
    copy = Home(SHOTS_HOME)
    _ = copy.manifest
    return copy


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
    app = app_factory(extra_env={"STEWARD_MOTION": "1", "STEWARD_FAKE_NOW": home.manifest["now"],
                                 "STEWARD_SYSINFO_FIXTURE": str(FIXTURE)})
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
