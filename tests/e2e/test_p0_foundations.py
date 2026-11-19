"""Phase 0 end-to-end scenarios (DESIGN.md §10, Phase 0)."""

import time

import pytest
from PIL import Image

pytestmark = pytest.mark.p0


@pytest.mark.smoke
def test_E0_01_launch_headless(app):
    state = app.state()
    assert state["windows"] == 1
    assert state["active_window"]["title"] == "Cairn"

    names = app.a11y_names()
    assert "Main toolbar" in names, f"no header bar in the accessibility tree: {sorted(names)}"
    assert "Sidebar" in names, f"no sidebar in the accessibility tree: {sorted(names)}"

    # G_DEBUG=fatal-criticals would have killed the app; check for warnings too.
    log = app.log_text()
    assert "CRITICAL" not in log, log
    assert app.running()


def test_E0_02_about_dialog_opens_and_closes(app):
    app.action("app.about")
    dialog = app.wait_for(lambda: app.state()["active_window"]["dialog"], "the About dialog")
    assert dialog["type"] == "about"
    assert dialog["application_name"] == "Cairn"
    assert dialog["version"] == app.state()["version"]

    # The main window has a "Close" button too; target the dialog's.
    app.a11y_click("Close", role="button", within="About")
    app.wait_for(lambda: app.state()["active_window"]["dialog"] is None,
                 "the About dialog to close")


def test_E0_03_screenshot_is_a_real_image(app, tmp_path):
    window = app.state()["active_window"]
    path = app.screenshot(tmp_path / "window.png")

    with Image.open(path) as image:
        assert image.size == (window["width"], window["height"])
        colours = image.convert("RGB").getcolors(maxcolors=1 << 20)
    assert colours is not None and len(colours) > 1, "screenshot is a single flat colour"


def test_E0_04_refuses_a_home_without_the_marker(session, home, app_factory):
    (home / ".cairn-fixture").unlink()
    before = home.snapshot()

    app = app_factory(wait=False)
    assert app.wait_exit(timeout=10) == 2
    assert "not a fixture home" in app.log_text()
    assert home.snapshot() == before, "the app touched a home that is not a fixture"


def test_harness_presses_real_keys(app):
    """Not a DESIGN.md scenario: proves the harness can press real keys (spike
    P0-S3, ADR 0003), which later phases rely on to test shortcuts."""
    app.press("ctrl+q")
    assert app.wait_exit(timeout=5) == 0


@pytest.mark.smoke
def test_E0_05_quit_exits_cleanly(app):
    started = time.monotonic()
    app.action("app.quit")
    assert app.wait_exit(timeout=2) == 0
    assert time.monotonic() - started < 2
