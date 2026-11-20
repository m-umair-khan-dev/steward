"""Performance pass (P6-T14, DESIGN.md §8.4): measures the targets that can be
measured in the container and writes artifacts/perf/app.json.

    make perf     (builds an optimised binary with the test interface, then runs this)

Measured: launch to a usable window (cold and warm), opening a folder of
10,000 items (listing done, and the first cards on screen), and memory with
one window, idle. Search per keystroke over a million entries is the E3-10
measurement (crates/core/tests/index.rs), run in release by `make perf`.
"""

import json
import os
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tests" / "e2e"))

from gi.repository import GLib  # noqa: E402

from harness import App, Home, Session, wait_for  # noqa: E402

OUT = REPO / "artifacts" / "perf"


def rss_mb(pid: int) -> float:
    for line in Path(f"/proc/{pid}/status").read_text().splitlines():
        if line.startswith("VmRSS:"):
            return int(line.split()[1]) / 1024
    return 0.0


def launch(session, home) -> tuple[App, float]:
    app = App(session, home)
    started = time.monotonic()
    app.launch(wait=False)
    wait_for(app._name_owned, "the D-Bus name", 30)
    wait_for(app._window_ready, "the window", 30)
    return app, (time.monotonic() - started) * 1000


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    results: dict = {}
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
        root = Path(tmp)
        source = root / "home"
        subprocess.run([sys.executable, "-I", str(REPO / "docker/scripts/make-fixture-home"), str(source)],
                       check=True, capture_output=True)
        big = source / "Documents" / "Ten thousand"
        big.mkdir()
        for n in range(10_000):
            (big / f"file-{n:05d}.txt").write_bytes(b"")
        home = Home(source)
        session = Session(root / "session", home.path)
        session.start()
        try:
            app, cold = launch(session, home)
            app.wait_idle(30)
            app.action("app.quit")
            app.wait_exit(10)
            warm = []
            for _ in range(5):
                app, ms = launch(session, home)
                warm.append(ms)
                app.action("app.quit")
                app.wait_exit(10)
            results["launch_ms"] = {"cold": round(cold), "warm_median": round(statistics.median(warm)),
                                    "warm": [round(w) for w in warm], "target": 500}

            app, _ = launch(session, home)
            app.wait_idle(30)
            loads, paints = [], []
            for _ in range(5):
                app.action("win.go-to", GLib.Variant("s", str(home / "Documents")))
                wait_for(lambda: app.folder()["state"] == "ready", "Documents", 10)
                started = time.monotonic()
                app.action("win.go-to", GLib.Variant("s", str(big)))
                wait_for(lambda: (f := app.folder())["path"] == str(big) and f["bound_cards"] > 0, "cards", 20)
                paints.append((time.monotonic() - started) * 1000)
                wait_for(lambda: app.folder()["state"] == "ready", "the listing", 20)
                loads.append(app.folder()["last_load_ms"])
            results["folder_10000"] = {"first_cards_ms_median": round(statistics.median(paints)),
                                       "listing_ms_median": statistics.median(loads),
                                       "first_cards_ms": [round(p) for p in paints], "target_first_paint": 150}
            app.action("win.go-to", GLib.Variant("s", str(home)))
            app.wait_idle(30)
            time.sleep(3)  # noqa: perf script, not a test: let caches settle before reading memory
            results["memory_idle_mb"] = {"rss": round(rss_mb(app.process.pid), 1), "target": 150}
            app.action("app.quit")
            app.wait_exit(10)
        finally:
            session.stop()
    (OUT / "app.json").write_text(json.dumps(results, indent=2) + "\n")
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
