"""E6-07 (metadata) and the desktop file's actions (P6-T06)."""

import re
import subprocess
from pathlib import Path

import pytest

pytestmark = pytest.mark.p6

REPO = Path(__file__).resolve().parents[2]
DATA = REPO / "data"


def test_E6_07_desktop_file_and_appstream_validate():
    desktop = subprocess.run(["desktop-file-validate", str(DATA / "dev.cairn.Files.desktop")],
                             capture_output=True, text=True)
    assert desktop.returncode == 0 and "error" not in desktop.stdout.lower(), desktop.stdout
    appstream = subprocess.run(["appstreamcli", "validate", "--no-net", str(DATA / "dev.cairn.Files.metainfo.xml")],
                               capture_output=True, text=True)
    assert appstream.returncode == 0, appstream.stdout + appstream.stderr


def test_E6_07_the_metadata_names_this_release():
    version = re.search(r'^version = "([^"]+)"', (REPO / "Cargo.toml").read_text(), re.M).group(1)
    metainfo = (DATA / "dev.cairn.Files.metainfo.xml").read_text()
    assert f'<release version="{version}"' in metainfo
    assert (REPO / "site/src/content/changelog" / f"{version}.md").exists()


def run_cairn(app, *args: str) -> subprocess.CompletedProcess:
    """A second launch, as the desktop file's actions do it."""
    return subprocess.run([app.binary, *args], env=app.env(), capture_output=True, text=True, timeout=20)


def test_desktop_actions_reach_the_running_window(app, home):
    second = run_cairn(app, "--free-up-space")
    assert second.returncode == 0, second.stderr
    app.wait_for(lambda: app.state()["tools"]["page"] == "clean", "Free Up Space")
    run_cairn(app, "--new-workspace")
    app.wait_for(lambda: (d := app.state()["active_window"]["dialog"]) and d["type"] == "workspace-new",
                 "the new-workspace dialog")
    app.press("escape")
    run_cairn(app, str(home / "Documents" / "Notes.md"))
    app.wait_for(lambda: app.folder()["path"] == str(home / "Documents")
                 and app.folder()["selection"] == ["Notes.md"], "the file, selected in its folder")
    assert app.state()["tools"]["page"] == ""
