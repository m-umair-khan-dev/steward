"""Phase 5 end-to-end scenarios E5-08 and E5-09: Installed."""

import json
from pathlib import Path

import pytest
from gi.repository import GLib

from harness.fakes import FakeTools

pytestmark = pytest.mark.p5

DATA = Path(__file__).resolve().parents[1] / "data" / "packages"
DPKG_FORMAT = "-f=${Package}\t${Version}\t${Installed-Size}\t${db:Status-Status}\t${binary:Summary}\n"


def s(text: str) -> GLib.Variant:
    return GLib.Variant("s", text)


def recorded(name: str) -> str:
    return (DATA / name).read_text()


def record_every_tool(fakes: FakeTools) -> None:
    """Each tool answers with its recorded output (tests/data/packages)."""
    fakes.answer("dpkg-query", ["-W", DPKG_FORMAT], recorded("dpkg-query-list.txt"))
    fakes.answer("dpkg-query", ["-S"], recorded("dpkg-query-owners.txt"))
    fakes.answer("apt-mark", ["showmanual"], recorded("apt-mark-showmanual.txt"))
    fakes.answer("snap", ["list"], recorded("snap-list.txt"))
    fakes.answer("flatpak", ["list", "--app"], recorded("flatpak-apps.txt"))
    fakes.answer("flatpak", ["list", "--runtime"], recorded("flatpak-runtimes.txt"))
    fakes.answer("npm", ["ls"], recorded("npm-ls.json"))
    fakes.answer("npm", ["root", "-g"], "/nonexistent/lib/node_modules\n")
    fakes.tool("pipx")
    fakes.answer("pip", ["list", "--user", "--not-required"], recorded("pip-list-not-required.json"))
    fakes.answer("pip", ["list", "--user"], recorded("pip-list.json"))
    fakes.answer("cargo", ["install", "--list"], recorded("cargo-install-list.txt"))
    fakes.answer("gem", ["list", "--local"], recorded("gem-list.txt"))


def tool_homes(tmp_path: Path) -> dict:
    """A Cargo home with install records and a pipx home with the recorded
    venvs, as the environment the app sees."""
    cargo = tmp_path / "cargo-home"
    cargo.mkdir()
    (cargo / ".crates.toml").write_text("[v1]\n")
    pipx = tmp_path / "pipx-home"
    for venv, info in json.loads(recorded("pipx-list.json"))["venvs"].items():
        (pipx / "venvs" / venv).mkdir(parents=True)
        (pipx / "venvs" / venv / "pipx_metadata.json").write_text(json.dumps(info["metadata"]))
    return {"CARGO_HOME": str(cargo), "PIPX_HOME": str(pipx)}


def installed(app) -> dict:
    return app.state()["installed"]


def open_installed(app) -> dict:
    app.action("win.tool-open", s("installed"))
    return app.wait_for(lambda: (i := installed(app)) and i["loaded"] and not i["looking"] and i,
                        "every tool to answer", timeout=30)


def test_E5_08_installed_from_recorded_output(session, home, app_factory, tmp_path):
    fakes = FakeTools(tmp_path / "fake-bin")
    record_every_tool(fakes)
    app = app_factory(extra_env={"PATH": fakes.path(session.base_env()["PATH"]), **tool_homes(tmp_path)})
    state = open_installed(app)
    expected = json.loads(recorded("expected.json"))
    assert state["tools"] == expected["tools"], "per-tool counts and sizes"
    assert app.state()["tools"]["page"] == "installed"

    app.action("win.installed-filter", s("you"))
    app.wait_for(lambda: installed(app)["filter"] == "you", "the filter")
    assert sorted(installed(app)["shown"]) == expected["filters"]["you"], "You installed"
    app.action("win.installed-filter", s("app"))
    app.wait_for(lambda: installed(app)["filter"] == "app", "the filter")
    assert sorted(installed(app)["shown"]) == expected["filters"]["app"]
    app.action("win.installed-filter", s("all"))
    app.wait_for(lambda: len(installed(app)["shown"]) == expected["total"], "every package")

    app.action("win.installed-copy", s("apt:git"))
    app.wait_for(lambda: installed(app)["copied"] == "sudo apt remove git", "the remove command")

    saved = home / "Documents" / "Installed packages.txt"
    app.action("win.installed-save", s(str(saved)))
    app.wait_for(saved.exists, "the saved list")
    assert saved.read_text() == recorded("installed-list.golden.txt")
    assert "Installed" in app.a11y_names()
    calls = fakes.calls()
    assert not [c for c in calls if c[0] in ("sudo", "pkexec")], "nothing ran as administrator"


def test_E5_09_tools_that_are_not_installed_are_not_shown(session, home, app_factory, tmp_path):
    fakes = FakeTools(tmp_path / "fake-bin")
    record_every_tool(fakes)
    for tool in ("dpkg-query", "apt-mark", "npm", "pipx", "pip", "cargo", "gem"):
        fakes.remove(tool)
    # Only the stand-ins are on PATH: the container's own dpkg and cargo are not.
    app = app_factory(extra_env={"PATH": str(fakes.folder), **tool_homes(tmp_path)})
    state = open_installed(app)
    assert set(state["tools"]) == {"snap", "flatpak"}
    toast = app.state()["last_toast"] or ""
    assert "could not" not in toast.lower() and "error" not in toast.lower(), toast
    assert "could not run" not in app.log_text()
