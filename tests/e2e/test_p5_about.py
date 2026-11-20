"""Phase 5 end-to-end scenarios E5-10 and E5-11: About This Computer, and
the read-only proof for every Phase 5 page."""

from pathlib import Path

import pytest
from gi.repository import GLib

pytestmark = pytest.mark.p5

DATA = Path(__file__).resolve().parents[1] / "data" / "sysinfo"


def s(text: str) -> GLib.Variant:
    return GLib.Variant("s", text)


def about(app) -> dict:
    return app.state()["about"]


@pytest.fixture
def about_app(session, home, app_factory):
    return app_factory(extra_env={"CAIRN_SYSINFO_FIXTURE": str(DATA / "fixture.json"),
                                  "CAIRN_FAKE_NOW": home.manifest["now"], "TZ": "UTC"})


def test_E5_10_about_with_fixture_probes(about_app):
    app = about_app
    app.a11y_click("Computer")
    app.wait_for(lambda: app.state()["tools"]["page"] == "about", "About This Computer")
    state = app.wait_for(lambda: (a := about(app)) and a["loaded"] and a, "the facts")
    rows = state["rows"]
    assert state["title"] == "ThinkPad X1 Carbon Gen 13"
    assert "Ubuntu 26.10" in state["subtitle"] and "Intel(R) Core(TM) Ultra 7 268V" in state["subtitle"]
    expected = {
        "battery/Condition": "Normal", "battery/Maximum capacity": "97%", "battery/Cycles": "110",
        "disk/File system": "ext4", "disk/Encrypted": "Yes (LUKS)", "disk/Drive": "Example NVMe 1TB",
        "memory/Pressure": "Low", "memory/Thermal state": "Normal (48 °C)",
        "displays/Built-in display": "2880 × 1800 · 120 Hz · 200%",
        "displays/Example 27 Monitor": "3840 × 2160 · 60 Hz · 150%",
        "security/Disk encryption": "On", "security/Firewall": "Off", "security/Secure Boot": "On",
        "security/AppArmor": "On", "security/Automatic security updates": "On",
        "computer/Computer name": "studio-desk", "computer/User": "sam", "computer/Cores": "8",
        "computer/Architecture": "x86_64", "software/Kernel": "7.3.0-10-generic", "software/Shell": "bash",
        "software/Region": "Europe/Lisbon · en_GB", "software/Packages": "1850 apt · 12 Snap · 4 Flatpak",
    }
    for key, value in expected.items():
        assert rows[key] == value, key
    chips = {c["label"]: c["value"] for c in state["chips"]}
    assert chips == {"Up": "6 days, 15 hours", "In use": "1 year, 8 months", "Battery": "Normal"}
    assert "Computer name: studio-desk" in app.a11y_names(showing_only=False)

    app.action("win.about-copy")
    app.wait_for(lambda: about(app)["copied"], "Copy as Text")
    assert about(app)["copied"] == (DATA / "about.golden.txt").read_text()

    app.action("win.about-hide-names", s("true"))
    app.wait_for(lambda: about(app)["hide_names"], "Hide Names")
    rows = about(app)["rows"]
    assert rows["computer/Computer name"] != "studio-desk" and rows["computer/User"] != "sam"
    app.action("win.about-copy")
    app.wait_for(lambda: "studio-desk" not in (about(app)["copied"] or "studio-desk"), "a masked copy")
    assert "sam" not in about(app)["copied"].split()
    assert not any("studio-desk" in name for name in app.a11y_names(showing_only=False))


def test_E5_11_reading_changes_nothing(session, home, app_factory, tmp_path):
    before = home.snapshot(ignore_app_owned=True)
    app = app_factory(extra_env={"CAIRN_SYSINFO_FIXTURE": str(DATA / "fixture.json"),
                                 "CAIRN_FAKE_NOW": home.manifest["now"]})
    app.action("win.tool-open", s("clean"))
    app.wait_for(lambda: (c := app.state()["clean"]) and c["cards"] and not c["looking"] and c["disk"],
                 "Free Up Space", timeout=60)
    app.action("win.clean-review", s("dev_caches"))
    app.action("win.tool-open", s("installed"))
    app.wait_for(lambda: (i := app.state()["installed"]) and i["loaded"] and not i["looking"], "Installed",
                 timeout=60)
    app.action("win.tool-open", s("about"))
    app.wait_for(lambda: about(app)["loaded"], "About")
    app.wait_idle()
    app.action("app.quit")
    app.wait_exit(10)
    assert home.snapshot(ignore_app_owned=True) == before, "the fixture home is unchanged"
