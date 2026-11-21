"""Phase 7 end-to-end scenarios: developer tools (E7-01 to E7-03)."""

import pytest

pytestmark = pytest.mark.p7

WEB_APP = "Documents/Projects/web-app"


def git(app):
    return app.state()["path_bar"]["git"]


def test_E7_01_the_git_chip(app, home):
    facts = home.manifest["git"][WEB_APP]
    project = home / WEB_APP
    before = home.snapshot(WEB_APP)
    app.open_folder(project)
    chip = app.wait_for(lambda: git(app), "the Git chip")
    assert chip["branch"] == facts["branch"] and chip["changes"] == facts["changes"]
    assert f"Git branch {facts['branch']}, {facts['changes']} uncommitted changes" in app.a11y_names()
    app.open_folder(project / "src")
    app.wait_for(lambda: (g := git(app)) and g["root"] == str(project), "the chip in a subfolder")
    app.wait_idle()
    assert home.snapshot(WEB_APP) == before, "the repository is byte-for-byte unchanged"


def test_E7_02_no_chip_and_no_crash(app, home):
    app.open_folder(home / "Documents" / "Invoices")
    app.wait_idle()
    assert git(app) is None, "not a repository"
    school = home / "Documents" / "School"
    (school / ".git").mkdir()
    (school / ".git" / "HEAD").write_text("not a ref at all\n")
    app.open_folder(school)
    app.wait_idle()
    assert git(app) is None, "a corrupt .git gives no chip"
    assert app.running()


def install_fake_app(home, fakes, desktop_id: str, program: str, name: str) -> None:
    """A stand-in app: a desktop file in the test home whose Exec is a fake
    program that logs how it was started."""
    fakes.tool(program)
    apps = home / ".local" / "share" / "applications"
    apps.mkdir(parents=True, exist_ok=True)
    (apps / desktop_id).write_text(f"[Desktop Entry]\nType=Application\nName={name}\n"
                                   f"Exec={fakes.folder / program} %F\n")


def test_E7_03_open_in_terminal_and_editor(session, home, app_factory, tmp_path):
    from harness.fakes import FakeTools

    fakes = FakeTools(tmp_path / "fake-bin")
    install_fake_app(home, fakes, "org.gnome.Terminal.desktop", "gnome-terminal", "Terminal")
    install_fake_app(home, fakes, "code.desktop", "code", "Visual Studio Code")
    app = app_factory()
    info = app.state()["openin"]
    assert {t["id"] for t in info["terminals"]} == {"gnome-terminal"}
    assert {e["id"] for e in info["editors"]} == {"vscode"}
    project = home / WEB_APP
    app.open_folder(project)

    app.press("f4")
    app.wait_for(lambda: fakes.calls("gnome-terminal"), "the terminal to start")
    assert fakes.calls("gnome-terminal")[-1] == [f"--working-directory={project}"]

    app.press("ctrl+l")
    app.wait_for(lambda: app.state()["path_bar"]["editing"], "the path field")
    app.type_text("code .")
    app.press("return")
    app.wait_for(lambda: fakes.calls("code"), "the editor to start")
    assert fakes.calls("code")[-1] == [str(project)]
    assert app.folder()["path"] == str(project), "the command did not navigate"


def test_the_item_menu_opens_from_the_keyboard(app, home):
    """Shift+F10 (and the Menu key) open the selection's menu, with Open In."""
    app.open_folder(home / "Documents" / "Projects")
    app.select("web-app")
    app.press("shift+f10")
    app.wait_for(lambda: {"Open in Terminal", "Open in Editor"} <= app.a11y_names(), "the item menu")
