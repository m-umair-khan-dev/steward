"""Development screenshots of several screens, for visual checks against the
reference captures (ADR 0008). Writes artifacts/shots/<label>-*.png.

    docker compose -f compose.desktop.yaml run --rm e2e docker/scripts/run-e2e python3 tests/spikes/shots.py
"""

import json
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tests" / "e2e"))

from gi.repository import GLib  # noqa: E402

from harness import App, Home, Session, wait_for  # noqa: E402

OUT = REPO / "artifacts" / "shots"
OUT.mkdir(parents=True, exist_ok=True)
label = sys.argv[1] if len(sys.argv) > 1 else "dev"


def settle(app):
    app.wait_idle()
    # Let thumbnails and folder counts arrive.
    try:
        wait_for(lambda: all(c["thumbnail"] or not c["name"].lower().endswith((".png", ".jpg"))
                             for c in app.folder()["cards"]), "thumbnails", timeout=8)
    except AssertionError:
        pass
    app.wait_idle()


with tempfile.TemporaryDirectory() as tmp:
    work = Path(tmp)
    subprocess.run([sys.executable, "-I", str(REPO / "docker/scripts/make-fixture-home"),
                    str(work / "home")], check=True, capture_output=True)
    home = Home(work / "home")
    session = Session(work / "session", home.path)
    session.start()
    app = App(session, home)
    try:
        app.launch()
        settle(app)
        app.screenshot(OUT / f"{label}-home.png")

        app.action("win.toggle-preview")
        app.open_folder(home / "Downloads")
        settle(app)
        app.screenshot(OUT / f"{label}-downloads.png")

        app.action("win.group-by", GLib.Variant("s", "kind"))
        settle(app)
        app.screenshot(OUT / f"{label}-downloads-kind.png")

        app.action("win.group-by", GLib.Variant("s", "none"))
        app.open_folder(home / "Pictures" / "Holiday 2026")
        settle(app)
        app.select("IMG_4101.jpg")
        app.wait_idle()
        app.screenshot(OUT / f"{label}-pictures-selected.png")

        app.action("win.toggle-sidebar")
        app.open_folder(home / "Downloads")
        settle(app)
        app.screenshot(OUT / f"{label}-collapsed.png")
        app.action("win.toggle-sidebar")

        app.action("app.color-scheme", GLib.Variant("s", "dark"))
        app.open_folder(home / "Downloads")
        settle(app)
        app.screenshot(OUT / f"{label}-downloads-dark.png")
        (OUT / f"{label}-state.json").write_text(json.dumps(app.state(), indent=1))
        print("shots written to", OUT)
    finally:
        app.stop()
        session.stop()
