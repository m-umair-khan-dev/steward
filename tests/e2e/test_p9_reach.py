"""Phase 9 end-to-end scenarios: reach (E9-01 onwards)."""

import pytest
from gi.repository import GLib

pytestmark = pytest.mark.p9


def s(text: str) -> GLib.Variant:
    return GLib.Variant("s", text)


def test_E9_01_in_a_sandbox_features_say_why(session, home, app_factory):
    app = app_factory(extra_env={"CAIRN_SIMULATE_SANDBOX": "1"})
    assert app.state()["sandbox"] == "a sandbox"

    # Installed: an explanation instead of an empty list; no tool was asked.
    app.action("win.tool-open", s("installed"))
    app.wait_for(lambda: app.state()["tools"]["page"] == "installed", "Installed")
    installed = app.state()["installed"]
    assert installed["unavailable"] and "Cairn runs in a sandbox" in installed["unavailable"]
    assert not installed["loaded"] and not installed["looking"]
    assert installed["unavailable"] in app.a11y_names()

    # Smart Clean: the cards that need the host say so; the others work.
    app.action("win.tool-open", s("clean"))
    clean = app.wait_for(lambda: (c := app.state()["clean"]) and not c["looking"] and c, "Smart Clean", timeout=60)
    assert sorted(clean["unavailable"]) == ["installers", "leftovers"]
    assert clean["admin_unavailable"] and clean["admin"] == []
    assert clean["cards"]["duplicates"]["items"], "a card on the user's own files still works"
    # Further down the page: present, though not on screen.
    assert clean["admin_unavailable"] in app.a11y_names(showing_only=False)

    # Everything else works: browsing.
    app.open_folder(home / "Documents")
    assert app.folder()["path"] == str(home / "Documents")


# --- The browser extension's native host (P9-T04) ----------------------------------------

def test_the_browser_extension_host_saves_a_link(app, home, tmp_path):
    """The extension's host, fed a message as a browser sends it (a length,
    then JSON), runs `cairn add-link` against the running app. The browser
    side is a manual check (DESIGN.md P9-T04)."""
    import json
    import os
    import struct
    import subprocess
    from pathlib import Path
    host = Path(__file__).resolve().parents[2] / "browser-extension" / "host" / "cairn-native-host"
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    (bin_dir / "cairn").symlink_to(app.binary)
    env = app.env() | {"PATH": f"{bin_dir}:{os.environ.get('PATH', '/usr/bin:/bin')}"}

    def ask(message: dict) -> dict:
        data = json.dumps(message).encode()
        done = subprocess.run([str(host)], input=struct.pack("=I", len(data)) + data,
                              env=env, capture_output=True, timeout=30)
        (length,) = struct.unpack("=I", done.stdout[:4])
        return json.loads(done.stdout[4:4 + length])

    link = "http://127.0.0.1:9/from-the-browser"
    assert ask({"url": link})["ok"]
    app.wait_for(lambda: any(b["url"] == link for b in app.state()["bookmarks"]["all"]), "the link in Bookmarks")
    refused = ask({"url": "file:///etc/passwd"})
    assert not refused["ok"] and refused["error"] == "not a web address"
