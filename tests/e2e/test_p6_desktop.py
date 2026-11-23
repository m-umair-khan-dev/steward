"""E6-07 (metadata) and the desktop file's actions (P6-T06)."""

import re
import subprocess
from pathlib import Path

import pytest

pytestmark = pytest.mark.p6

REPO = Path(__file__).resolve().parents[2]
DATA = REPO / "data"


def test_E6_07_desktop_file_and_appstream_validate():
    desktop = subprocess.run(["desktop-file-validate", str(DATA / "io.github.m_umair_khan_dev.steward.desktop")],
                             capture_output=True, text=True)
    assert desktop.returncode == 0 and "error" not in desktop.stdout.lower(), desktop.stdout
    appstream = subprocess.run(["appstreamcli", "validate", "--no-net", str(DATA / "io.github.m_umair_khan_dev.steward.metainfo.xml")],
                               capture_output=True, text=True)
    assert appstream.returncode == 0, appstream.stdout + appstream.stderr


def test_E6_07_the_metadata_names_this_release():
    version = re.search(r'^version = "([^"]+)"', (REPO / "Cargo.toml").read_text(), re.M).group(1)
    metainfo = (DATA / "io.github.m_umair_khan_dev.steward.metainfo.xml").read_text()
    assert f'<release version="{version}"' in metainfo
    assert (REPO / "site/src/content/changelog" / f"{version}.md").exists()


def run_steward(app, *args: str) -> subprocess.CompletedProcess:
    """A second launch, as the desktop file's actions do it."""
    return subprocess.run([app.binary, *args], env=app.env(), capture_output=True, text=True, timeout=20)


def test_desktop_actions_reach_the_running_window(app, home):
    second = run_steward(app, "--free-up-space")
    assert second.returncode == 0, second.stderr
    app.wait_for(lambda: app.state()["tools"]["page"] == "clean", "Free Up Space")
    run_steward(app, "--new-workspace")
    app.wait_for(lambda: (d := app.state()["active_window"]["dialog"]) and d["type"] == "workspace-new",
                 "the new-workspace dialog")
    app.press("escape")
    run_steward(app, str(home / "Documents" / "Notes.md"))
    app.wait_for(lambda: app.folder()["path"] == str(home / "Documents")
                 and app.folder()["selection"] == ["Notes.md"], "the file, selected in its folder")
    assert app.state()["tools"]["page"] == ""


def test_E6_09_show_items_with_the_option_on(app, home):
    from gi.repository import Gio, GLib

    app.action("app.preferences")
    app.wait_for(lambda: app.state()["active_window"]["dialog"], "Preferences")
    app.a11y_click("Use as default file manager", within="Preferences")
    app.press("escape")
    target = home / "Downloads" / "Invoice-0923.pdf"

    def owned() -> bool:
        reply = app.bus.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
                                  "NameHasOwner", GLib.Variant("(s)", ("org.freedesktop.FileManager1",)),
                                  None, Gio.DBusCallFlags.NONE, 2000, None)
        return reply.unpack()[0]

    app.wait_for(owned, "Steward to answer as the file manager")
    app.bus.call_sync("org.freedesktop.FileManager1", "/org/freedesktop/FileManager1",
                      "org.freedesktop.FileManager1", "ShowItems",
                      GLib.Variant("(ass)", ([target.as_uri()], "")), None, Gio.DBusCallFlags.NONE, 5000, None)
    app.wait_for(lambda: app.folder()["path"] == str(target.parent)
                 and app.folder()["selection"] == [target.name], "the file, selected in its folder")


def test_report_a_bug_fills_in_version_and_release(app):
    """P6-T13: the address has the version and the Ubuntu release."""
    from urllib.parse import parse_qs, urlparse

    app.action("app.report-bug")
    url = app.wait_for(lambda: app.state()["last_report"], "the issue tracker address")
    body = parse_qs(urlparse(url).query)["body"][0]
    assert url.split("?")[0].endswith("/issues/new")
    assert f"**Steward:** {app.state()['version']}" in body
    release = next(line.split("=", 1)[1].strip().strip('"') for line in open("/etc/os-release")
                   if line.startswith("PRETTY_NAME="))
    assert release in body
