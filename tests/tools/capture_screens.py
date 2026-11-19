"""Captures review screenshots of the app against a fresh fixture home, for
comparison with the reference captures (P1-T16, HUMAN sign-off).

    make shell, then: docker/scripts/run-e2e python3 tests/tools/capture_screens.py

Writes artifacts/screens/*.png, including frames taken while files fly into
their kind folders.
"""

import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tests" / "e2e"))

from gi.repository import GLib  # noqa: E402

from harness import App, Home, Session, wait_for  # noqa: E402

OUT = REPO / "artifacts" / "screens"


def s(text: str) -> GLib.Variant:
    return GLib.Variant("s", text)


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        root = Path(tmp)
        source = root / "home"
        subprocess.run([sys.executable, "-I", str(REPO / "docker/scripts/make-fixture-home"), str(source)],
                       check=True, capture_output=True)
        home = Home(source)
        session = Session(root / "session", home.path)
        session.start()
        app = App(session, home, extra_env={"CAIRN_MOTION": "1"})
        try:
            app.launch()
            app.open_folder(home / "Downloads")
            app.action("win.toggle-preview")
            app.wait_idle()
            wait_for(lambda: True, "settle", 0.5)
            app.screenshot(OUT / "01-downloads-grid.png")

            app.action("win.group-by", s("kind"))
            wait_for(lambda: app.state()["flying"] > 0, "cards to take off", 3)
            for frame in range(4):
                app.screenshot(OUT / f"02-flight-{frame}.png")
            wait_for(lambda: app.state()["flying"] == 0, "cards to land", 5)
            app.wait_idle()
            app.screenshot(OUT / "03-by-kind.png")

            app.action("win.group-by", s("none"))
            app.select("Statement-0923.pdf")
            app.wait_idle()
            app.screenshot(OUT / "04-selected.png")

            app.action("win.view-mode", s("list"))
            app.wait_idle()
            app.screenshot(OUT / "05-list.png")

            app.action("win.view-mode", s("grid"))
            app.open_folder(home.path)
            app.action("app.color-scheme", s("dark"))
            wait_for(lambda: app.state()["dark"], "dark style", 5)
            app.wait_idle()
            app.screenshot(OUT / "06-home-dark.png")

            app.action("app.color-scheme", s("light"))
            app.action("win.toggle-sidebar")
            app.wait_idle()
            app.screenshot(OUT / "07-rail.png")
            app.action("win.toggle-sidebar")

            app.action("app.about")
            app.wait_idle()
            app.screenshot(OUT / "08-about.png")
        finally:
            app.stop()
            session.stop()
    print("\n".join(sorted(str(p.relative_to(REPO)) for p in OUT.glob("*.png"))))


if __name__ == "__main__":
    main()
