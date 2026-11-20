"""Phase 5 end-to-end scenarios E5-01 to E5-07: Free Up Space (Smart Clean)."""

import re
import subprocess

import pytest
from gi.repository import GLib

from harness.fakes import FakeTools

pytestmark = pytest.mark.p5

KERNELS = ("linux-image-7.0.0-38-generic\t7.0.0-38.38\t15000\tinstalled\n"
           "linux-image-7.3.0-10-generic\t7.3.0-10.10\t16100\tinstalled\n")
SNAPS_ALL = ("Name     Version   Rev   Tracking       Publisher  Notes\n"
             "core22   20250110  1748  latest/stable  canonical  base,disabled\n"
             "core22   20250315  1802  latest/stable  canonical  base\n")
SNAPS = ("Name     Version   Rev   Tracking       Publisher  Notes\n"
         "core22   20250315  1802  latest/stable  canonical  base\n")


def s(text: str) -> GLib.Variant:
    return GLib.Variant("s", text)


def system_as_the_manifest_says(fakes: FakeTools) -> None:
    """photo-tool 3.2.0 installed, one Flatpak, no app Snaps, a 1.2G journal."""
    fakes.answer("dpkg-query", ["-W", "-f=${db:Status-Status} ${Version}", "photo-tool"], "installed 3.2.0")
    fakes.answer("dpkg-query", ["-W", "-f=${Package}\t${Version}\t${Installed-Size}\t${db:Status-Status}\n",
                                "linux-image-[0-9]*"], KERNELS)
    fakes.answer("dpkg-query", ["-W", "-f=${Installed-Size}\n"], "1024\n2048\n")
    fakes.answer("flatpak", ["list"], "org.example.Kept\n")
    fakes.answer("snap", ["list", "--all"], SNAPS_ALL)
    fakes.answer("snap", ["list"], SNAPS)
    fakes.answer("journalctl", ["--disk-usage"],
                 "Archived and active journals take up 1.2G in the file system.\n")
    for tool in ("sudo", "pkexec", "apt", "apt-get"):
        fakes.tool(tool)


@pytest.fixture
def fakes(tmp_path) -> FakeTools:
    tools = FakeTools(tmp_path / "fake-bin")
    system_as_the_manifest_says(tools)
    return tools


@pytest.fixture
def clean_app(session, home, app_factory, fakes):
    now = home.manifest["now"]
    return app_factory(extra_env={"PATH": fakes.path(session.base_env()["PATH"]), "CAIRN_FAKE_NOW": now})


def clean(app) -> dict:
    return app.state()["clean"]


def open_clean(app, timeout: float = 60) -> dict:
    app.action("win.tool-open", s("clean"))
    return app.wait_for(lambda: (c := clean(app)) and c["cards"] and not c["looking"] and c["disk"]
                        and c["admin"] is not None and c, "every card to be filled in", timeout=timeout)


def relative(home, path: str) -> str:
    return str(__import__("pathlib").Path(path).relative_to(home.path))


def rows(home, items: list) -> list:
    return sorted((relative(home, i["path"]), i["bytes"], i["ticked"], i["keep"]) for i in items)


def expected(home, card: str) -> list:
    return sorted((i["path"], i["bytes"], i["ticked"], i.get("keep", False))
                  for i in home.manifest["clean"]["cards"][card])


def test_E5_01_cards_look_then_fill(session, home, app_factory, fakes):
    app = app_factory(extra_env={"PATH": fakes.path(session.base_env()["PATH"]),
                                 "CAIRN_FAKE_NOW": home.manifest["now"], "CAIRN_CLEAN_DELAY_MS": "1500"})
    app.action("win.tool-open", s("clean"))
    first = app.wait_for(lambda: (c := clean(app)) and c["cards"] and c, "the cards")
    assert app.state()["tools"]["page"] == "clean"
    assert len(first["cards"]) == 8, "every card appears at once"
    assert all(card["state"] == "looking" for card in first["cards"].values()), "all start as Looking…"
    state = app.wait_for(lambda: (c := clean(app)) and not c["looking"] and c, "the cards to fill", timeout=60)
    for card, totals in home.manifest["clean"]["totals"].items():
        assert state["cards"][card]["total"] == totals, card
    assert state["ticked_bytes"] == home.manifest["clean"]["ticked_bytes"], "the headline is the ticked sum"
    assert "Free Up Space" in app.a11y_names()


def test_E5_02_default_ticks(clean_app, home):
    state = open_clean(clean_app)
    for card in home.manifest["clean"]["cards"]:
        assert rows(home, state["cards"][card]["items"]) == expected(home, card), card
    for card, facts in state["cards"].items():
        if facts["group"] == "your-call":
            assert not any(i["ticked"] for i in facts["items"]), f"{card} is the user's call"
    ticked = {relative(home, i["path"]) for c in state["cards"].values() for i in c["items"] if i["ticked"]}
    caches = {i["path"] for i in home.manifest["clean"]["cards"]["dev_caches"]}
    assert caches <= ticked, "allow-listed caches are ticked"


def test_E5_03_a_folder_called_cache_is_never_listed(clean_app, home):
    state = open_clean(clean_app)
    listed = [i["path"] for c in state["cards"].values() for i in c["items"]]
    for path in home.manifest["clean"]["not_listed"]:
        folder = str(home / path)
        assert not any(p == folder or p.startswith(folder + "/") for p in listed), path


def test_E5_04_review_duplicates(clean_app, home):
    open_clean(clean_app)
    clean_app.action("win.clean-review", s("duplicates"))
    clean_app.wait_for(lambda: clean(clean_app)["review"] == "duplicates", "the review page")
    items = clean(clean_app)["cards"]["duplicates"]["items"]
    kept = [i for i in items if i["keep"]]
    sets = [g for g in home.manifest["duplicates"] if not any(p.startswith(".") for p in g)]
    assert len(kept) == len(sets), "one copy per set is marked Keep"
    assert str(home / "Documents/Invoices/Invoice-0923.pdf") in {i["path"] for i in kept}, \
        "a filed copy is kept in preference to one in Downloads"
    # A kept copy cannot be ticked.
    clean_app.action("win.clean-tick", s(kept[0]["path"]))
    clean_app.action("win.clean-tick", s(f"card:duplicates"))
    clean_app.wait_idle()
    after = clean(clean_app)["cards"]["duplicates"]["items"]
    assert not any(i["ticked"] for i in after if i["keep"])
    assert all(i["ticked"] for i in after if not i["keep"]), "Tick all ticks every spare copy"
    assert "Keep Invoice-0923.pdf" in clean_app.a11y_names()


def test_E5_05_move_to_trash_and_undo(clean_app, home):
    state = open_clean(clean_app)
    before = state["disk"]["trash"]
    caches = [home / i["path"] for i in home.manifest["clean"]["cards"]["dev_caches"]]
    assert len(caches) == 4
    clean_app.action("win.clean-review", s("dev_caches"))
    clean_app.wait_for(lambda: clean(clean_app)["review"] == "dev_caches", "the review page")
    clean_app.action("win.clean-trash", s("dev_caches"))
    clean_app.wait_idle(timeout=30)
    toast = clean_app.state()["last_toast"]
    assert re.fullmatch(r"Moved 4 items \(\d+(\.\d)?\sGB\) to the Trash", toast), toast
    total = sum(i["bytes"] for i in home.manifest["clean"]["cards"]["dev_caches"])
    assert GLib.format_size(total) in toast
    trash = home / ".local/share/Trash/files"
    assert all(not c.exists() for c in caches)
    assert {p.name for p in trash.iterdir()} >= {c.name for c in caches}
    clean_app.wait_for(lambda: (d := clean(clean_app)["disk"]) and d["trash"] > before,
                       "the Trash part of the bar to grow", timeout=30)
    clean_app.press("ctrl+z")
    clean_app.wait_idle(timeout=30)
    assert all(c.is_dir() for c in caches), "Undo restores everything"


def test_E5_06_a_symlink_in_a_cache_leaves_its_target_alone(clean_app, home):
    open_clean(clean_app)
    link = home / home.manifest["clean"]["cache_symlink"]["path"]
    assert link.is_symlink()
    documents = home.snapshot("Documents")
    clean_app.action("win.clean-trash", s("dev_caches"))
    clean_app.wait_idle(timeout=30)
    assert not link.parent.exists()
    moved = home / ".local/share/Trash/files" / link.parent.name / link.name
    assert moved.is_symlink(), "the link went to the Trash as a link"
    assert home.snapshot("Documents") == documents, "Documents is untouched"


def test_E5_07_needs_administrator_only_copies(session, clean_app, fakes):
    state = open_clean(clean_app)
    admin = {row["id"]: row for row in state["admin"]}
    assert {"apt-cache", "snap-revisions", "journal", "old-kernels"} <= set(admin)
    assert admin["journal"]["bytes"] == 1288490189
    assert admin["snap-revisions"]["command"] == "sudo snap remove core22 --revision=1748"
    wl_ok = True
    for row in admin.values():
        clean_app.action("win.clean-copy", s(row["id"]))
        clean_app.wait_for(lambda: clean(clean_app)["copied"] == row["command"], f"the {row['id']} command")
        if wl_ok:
            pasted = subprocess.run(["wl-paste", "--no-newline", "--type", "text/plain"], env=session.base_env(),
                                    capture_output=True, text=True, timeout=10)
            if pasted.returncode != 0:
                wl_ok = False  # no clipboard access for other clients here (see E2-08)
            else:
                assert pasted.stdout == row["command"], row["id"]
    calls = fakes.calls()
    assert not [c for c in calls if c[0] in ("sudo", "pkexec", "apt", "apt-get")], calls
    assert all(c[1:] in (["list"], ["list", "--all"]) for c in calls if c[0] == "snap"), calls
    assert all(c[1:] == ["--disk-usage"] for c in calls if c[0] == "journalctl"), calls
