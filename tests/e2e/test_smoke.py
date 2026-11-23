"""Smoke scenarios (E6-05): run in every e2e run, and against the installed
.deb by `make deb-test`. They use only what any client can use: the D-Bus
name, the exported actions, a second launch, and the accessibility tree."""

import subprocess

import pytest
from gi.repository import Gio, GLib

pytestmark = pytest.mark.smoke


def s(text: str) -> GLib.Variant:
    return GLib.Variant("s", text)


@pytest.fixture
def ready(app):
    app.dismiss_welcome()
    return app


def test_smoke_window_and_sidebar(ready):
    names = ready.a11y_names()
    assert {"Main toolbar", "Sidebar", "Free Up Space", "Installed"} <= names
    assert "CRITICAL" not in ready.log_text()


def test_smoke_open_a_folder(ready, home):
    ready.action("win.go-to", s(str(home / "Downloads")))
    ready.wait_for(lambda: "Invoice-0923.pdf" in ready.a11y_names(showing_only=False), "the Downloads folder")


def test_smoke_a_second_launch_reaches_the_window(ready):
    result = subprocess.run([ready.binary, "--free-up-space"], env=ready.env(), capture_output=True,
                            text=True, timeout=20)
    assert result.returncode == 0, result.stderr
    ready.wait_for(lambda: "Needs administrator" in ready.a11y_names(showing_only=False)
                   or "Safe to clear" in ready.a11y_names(showing_only=False), "Free Up Space")


def test_smoke_the_index_starts(ready):
    def owned() -> bool:
        reply = ready.bus.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
                                    "NameHasOwner", GLib.Variant("(s)", ("io.github.m_umair_khan_dev.steward.Indexer",)), None,
                                    Gio.DBusCallFlags.NONE, 2000, None)
        return reply.unpack()[0]

    ready.wait_for(owned, "the indexer", timeout=20)


def test_smoke_in_german(app_factory):
    app = app_factory(extra_env={"LANG": "de_DE.UTF-8", "LC_ALL": "de_DE.UTF-8", "LANGUAGE": "de"})
    if "Willkommen bei Steward" in app.a11y_names():
        app.a11y_click("Überspringen", role="button")
    app.wait_for(lambda: "Seitenleiste einklappen" in app.a11y_names(), "the German sidebar")
