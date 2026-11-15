"""Saves a screenshot of the app under the headless compositor, for visual
checks during development: artifacts/spikes/headless-<release>.png"""

import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tests" / "e2e"))

from harness import App, Home, Session  # noqa: E402

release = sys.argv[1] if len(sys.argv) > 1 else "dev"
with tempfile.TemporaryDirectory() as tmp:
    work = Path(tmp)
    subprocess.run([sys.executable, "-I", str(REPO / "docker/scripts/make-fixture-home"),
                    str(work / "home")], check=True, capture_output=True)
    session = Session(work / "session", work / "home")
    session.start()
    app = App(session, Home(work / "home"))
    try:
        app.launch()
        app.wait_idle()
        out = app.screenshot(REPO / "artifacts" / "spikes" / f"headless-{release}.png")
        import json
        state = app.state()
        (REPO / "artifacts" / "spikes" / f"state-{release}.json").write_text(json.dumps(state, indent=1))
        print(out, json.dumps(state["folder"], indent=1)[:1500])
    finally:
        app.stop()
        session.stop()
