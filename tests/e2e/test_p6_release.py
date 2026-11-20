"""Phase 6 end-to-end scenarios: first run and updates (E6-01, E6-02)."""

import pytest
from gi.repository import GLib

pytestmark = pytest.mark.p6


def s(text: str) -> GLib.Variant:
    return GLib.Variant("s", text)


def dialog(app):
    d = app.state()["active_window"]["dialog"]
    return d["type"] if d else None


def quit_app(app) -> None:
    app.action("app.quit")
    app.wait_exit(10)


def test_E6_01_welcome_shows_once(app_factory):
    app = app_factory(extra_env={"CAIRN_NO_WELCOME": ""})
    app.wait_for(lambda: dialog(app) == "welcome", "the welcome")
    assert app.state()["welcome"]["step"] == "demo"
    assert "Messy folder" in app.a11y_names()
    app.action("win.welcome-step", s("sort"))
    app.wait_for(lambda: app.state()["welcome"]["sorted"], "the demo to sort")
    app.wait_for(lambda: "Sorted folder" in app.a11y_names(), "the sorted state on screen")
    app.action("win.welcome-step", s("next"))
    app.wait_for(lambda: app.state()["welcome"]["step"] == "tour", "the tour")
    assert "Ctrl+K: Find anything" in app.a11y_names()
    app.action("win.welcome-step", s("next"))
    app.wait_for(lambda: app.state()["welcome"]["step"] == "yours", "make it yours")
    app.action("win.welcome-done")
    app.wait_for(lambda: dialog(app) is None, "the window")
    assert app.state()["welcome"]["shown"]
    quit_app(app)

    again = app_factory(extra_env={"CAIRN_NO_WELCOME": ""})
    again.wait_idle()
    assert dialog(again) is None, "the second start goes straight to the window"


def test_E6_02_whats_new_once_after_an_update(home, app_factory):
    # An earlier version ran before (settings in a key file the test can write).
    keyfile = home / ".config" / "glib-2.0" / "settings" / "keyfile"
    keyfile.parent.mkdir(parents=True, exist_ok=True)
    keyfile.write_text("[dev/cairn/Files]\nwelcome-shown=true\nlast-version='0.9.0'\n")
    env = {"CAIRN_NO_WELCOME": "", "GSETTINGS_BACKEND": "keyfile"}
    app = app_factory(extra_env=env)
    app.wait_for(lambda: dialog(app) == "whats-new", "What's New")
    assert app.state()["welcome"]["notes"] > 0
    assert any(name.startswith("What’s New in Cairn") for name in app.a11y_names())
    app.action("win.welcome-done")
    app.wait_for(lambda: dialog(app) is None, "the window")
    assert app.state()["welcome"]["last_version"] == app.state()["version"]
    quit_app(app)

    again = app_factory(extra_env=env)
    again.wait_idle()
    assert dialog(again) is None, "What's New shows once"


def test_E6_preferences_pages(app):
    """P6-T03: four pages; the shortcuts page lists every bound key."""
    app.action("app.preferences")
    app.wait_for(lambda: dialog(app) == "preferences", "Preferences")
    names = app.a11y_names()
    assert {"General", "Appearance", "Shortcuts", "Search and Index", "Default view", "Downloads folder"} <= names
    app.a11y_click("Appearance", within="Preferences")
    app.wait_for(lambda: "Accent" in app.a11y_names(), "Appearance")
    app.a11y_click("Shortcuts", within="Preferences")
    app.wait_for(lambda: any(n.startswith("Find anything") for n in app.a11y_names(showing_only=False)), "Shortcuts")
    listed = {n for n in app.a11y_names(showing_only=False) if ": " in n}
    for action, keys in app.state()["accels"].items():
        assert any(n.endswith(": " + keys[0]) for n in listed), f"{action} ({keys[0]}) is not listed"


# --- E6-03: translations ----------------------------------------------------------------

REPO = __import__("pathlib").Path(__file__).resolve().parents[2]
LANGUAGES = {
    # language: (LANG, LANGUAGE)
    "es": ("es_ES.UTF-8", "es"),
    "fr": ("fr_FR.UTF-8", "fr"),
    "de": ("de_DE.UTF-8", "de"),
    "ja": ("ja_JP.UTF-8", "ja"),
    "zh_CN": ("zh_CN.UTF-8", "zh_CN"),
    "en@pseudo": ("en_US.UTF-8", "en@pseudo"),
}


def translated(language: str, msgid: str) -> str:
    """The catalog's translation of a message (a probe that the UI uses it)."""
    import re
    text = (REPO / "po" / f"{language}.po").read_text()
    return eval(re.search(r'^msgid "' + re.escape(msgid) + r'"\nmsgstr (".*")$', text, re.M).group(1))  # noqa: S307


def test_E6_03_catalogs_translate_every_message():
    import subprocess
    result = subprocess.run(["python3", "-I", str(REPO / "docker/scripts/i18n"), "check"],
                            capture_output=True, text=True)
    assert result.returncode == 0, result.stdout


@pytest.mark.parametrize("language", list(LANGUAGES))
def test_E6_03_the_main_screens_in_each_language(app_factory, home, language):
    lang, catalog = LANGUAGES[language]
    collapse, tools = translated(language, "Collapse sidebar"), translated(language, "Free Up Space")
    app = app_factory(extra_env={"LANG": lang, "LC_ALL": lang, "LANGUAGE": catalog,
                                 "CAIRN_SYSINFO_FIXTURE": str(REPO / "tests/data/sysinfo/fixture.json")})
    out = REPO / "artifacts" / "i18n" / language
    out.mkdir(parents=True, exist_ok=True)
    app.wait_for(lambda: {collapse, tools} <= app.a11y_names(), f"the sidebar in {language}")
    app.open_folder(home / "Downloads")
    app.wait_idle()
    app.screenshot(out / "1-folder.png")
    app.action("win.tool-open", s("clean"))
    app.wait_for(lambda: (c := app.state()["clean"]) and c["cards"] and not c["looking"], "Free Up Space", 60)
    app.screenshot(out / "2-free-up-space.png")
    app.action("win.tool-open", s("about"))
    app.wait_for(lambda: app.state()["about"]["loaded"], "About")
    app.screenshot(out / "3-about.png")
    app.action("win.tool-close")
    app.action("app.preferences")
    app.wait_for(lambda: dialog(app) == "preferences", "Preferences")
    app.screenshot(out / "4-preferences.png")
    assert app.running(), "the app is still running"
    assert "Preferences" not in app.a11y_names(), "the Preferences dialog is translated"
