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


# --- AI Agents (E7-04, E7-05) ----------------------------------------------------------

from gi.repository import GLib  # noqa: E402


def s(text: str) -> GLib.Variant:
    return GLib.Variant("s", text)


def agents(app) -> dict:
    return app.state()["agents"]


def open_agents(app) -> dict:
    app.action("win.tool-open", s("agents"))
    return app.wait_for(lambda: (a := agents(app)) and a["loaded"] and a, "AI Agents", timeout=20)


def test_E7_04_ai_agents_match_the_manifest(session, home, app_factory):
    app = app_factory(extra_env={"CAIRN_FAKE_NOW": home.manifest["now"]})
    expected = home.manifest["agents"]
    state = open_agents(app)
    projects = {str(__import__("pathlib").Path(p["path"]).relative_to(home.path)): p for p in state["projects"]}
    assert set(projects) == set(expected["projects"])
    names = {"claude": "Claude Code", "codex": "Codex", "cursor": "Cursor"}
    for rel, facts in expected["projects"].items():
        got = projects[rel]
        assert got["tools"] == [names[t] for t in facts["tools"]], rel
        assert got["files"] == facts["files"], rel
        assert sorted(s["name"] for s in got["servers"]) == sorted(facts["servers"]), rel
        assert got["tokens"] == facts["tokens"], rel
    assert state["user_servers"] == len(expected["user_servers"])
    assert state["usage_total"] == expected["tokens_total"], "this week's tokens"
    assert "web-app" in app.a11y_names(showing_only=False)


def free_port() -> int:
    import socket
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return probe.getsockname()[1]


def test_E7_05_stop_a_local_server(session, home, app_factory):
    import subprocess
    import sys
    port = free_port()
    server = subprocess.Popen([sys.executable, "-m", "http.server", str(port), "--bind", "127.0.0.1"],
                              cwd=home / WEB_APP, env=session.base_env(),
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        app = app_factory()
        open_agents(app)
        listed = app.wait_for(lambda: next((x for x in agents(app)["servers"] if x["port"] == port), None),
                              "the server to be listed", timeout=15)
        assert listed["project"] == "web-app" and listed["pid"] == server.pid

        app.a11y_click(f"Stop the server on port {port}")
        app.wait_for(lambda: (d := app.state()["active_window"]["dialog"]) and d["type"] == "stop-server", "the question")
        app.a11y_click("Cancel", within="Stop the Server?")
        app.wait_for(lambda: app.state()["active_window"]["dialog"] is None, "the question to close")
        assert server.poll() is None, "cancel leaves it running"

        app.a11y_click(f"Stop the server on port {port}")
        app.wait_for(lambda: (d := app.state()["active_window"]["dialog"]) and d["type"] == "stop-server", "the question")
        app.a11y_click("Stop", role="button", within="Stop the Server?")
        app.wait_for(lambda: server.poll() is not None, "the server to end", timeout=10)
        app.wait_for(lambda: all(x["port"] != port for x in agents(app)["servers"]), "the list to update", timeout=10)
    finally:
        if server.poll() is None:
            server.kill()
