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
