"""E10-04: a long random run with the real pointer and keyboard.

Random clicks on whatever controls are showing, right-clicks, the mouse
wheel, keys and typing, from a seed. After every step the app must still
run, go idle within 10 s and log no QML error. Every step is written to
artifacts/e2e/monkey/steps.jsonl, so a failure can be replayed with the
same seed:

    make e2e k=E10_04 steps=3000 seed=1234

The gate runs 200 steps with a fixed seed.
"""

import json
import os
import random
from pathlib import Path

from harness.sweep import CLICKABLE, PROBLEM, SKIP

STEPS = int(os.environ.get("STEWARD_MONKEY_STEPS") or 200)
SEED = int(os.environ.get("STEWARD_MONKEY_SEED") or 20261124)
LOG = Path(__file__).resolve().parents[2] / "artifacts" / "e2e" / "monkey"

# Keys a person presses while browsing. Not Ctrl+Q: quitting is not a failure.
KEYS = ["escape", "tab", "shift+tab", "up", "down", "left", "right", "return", "space",
        "home", "end", "pageup", "pagedown", "backspace", "delete", "f2", "f3", "f9",
        "ctrl+1", "ctrl+2", "ctrl+h", "ctrl+z", "ctrl+shift+z", "ctrl+a", "ctrl+c", "ctrl+v",
        "ctrl+f", "ctrl+k", "ctrl+l", "ctrl+plus", "ctrl+minus", "ctrl+0", "alt+left",
        "alt+right", "alt+up", "ctrl+comma", "ctrl+shift+n", "ctrl+shift+p", "f5", "f6", "ctrl+u"]
TEXT = "abcdefghijklmnopqrstuvwxyz0123456789 ._-"


def clickables(tree: dict) -> list[dict]:
    found = []
    stack = [tree]
    while stack:
        node = stack.pop()
        ext = node.get("extents")
        if (node["role"] in CLICKABLE and node["showing"] and node["enabled"]
                and node["name"] not in SKIP and ext and ext[2] > 1 and ext[3] > 1
                and 0 <= ext[0] + ext[2] / 2 < 1280 and 0 <= ext[1] + ext[3] / 2 < 800):
            found.append(node)
        stack.extend(node["children"])
    return found


def test_E10_04_random_run(app):
    rng = random.Random(SEED)
    LOG.mkdir(parents=True, exist_ok=True)
    log = (LOG / "steps.jsonl").open("w")
    offset = len(app.log_text())
    history: list[dict] = []

    for number in range(STEPS):
        kind = rng.choices(["click", "right", "key", "type", "wheel"], [55, 8, 25, 6, 6])[0]
        step = {"n": number, "kind": kind}
        if kind == "click":
            targets = clickables(app.a11y_tree())
            if not app.state()["dialog"]:
                # The window's own Close button quits the app, as it should.
                targets = [t for t in targets if t["name"] != "Close"]
            if targets:
                node = rng.choice(targets)
                x, y, w, h = node["extents"]
                step |= {"role": node["role"], "name": node["name"]}
                app.click_at(x + w / 2, y + h / 2)
        elif kind == "right":
            x, y = rng.randrange(290, 1270), rng.randrange(150, 790)
            step |= {"at": [x, y]}
            app.click_at(x, y, button="right")
        elif kind == "key":
            step["key"] = rng.choice(KEYS)
            app.press(step["key"])
        elif kind == "type":
            step["text"] = "".join(rng.choice(TEXT) for _ in range(rng.randrange(1, 8)))
            app.type_text(step["text"])
        else:
            x, y, steps = rng.randrange(10, 1270), rng.randrange(10, 790), rng.choice([-5, -2, 2, 5])
            step |= {"at": [x, y], "steps": steps}
            app.scroll_at(x, y, steps)
        history.append(step)
        log.write(json.dumps(step) + "\n")
        log.flush()

        context = f"seed {SEED}, step {number}; the last steps:\n" + "\n".join(
            json.dumps(s) for s in history[-8:])
        assert app.running(), f"the app exited ({app.process.returncode}) at {context}"
        app.wait_idle(timeout=10)
        text = app.log_text()
        problems = [line for line in text[offset:].splitlines() if PROBLEM.search(line)]
        offset = len(text)
        assert not problems, "QML problems: " + "\n".join(problems[:5]) + f"\nat {context}"
    log.close()
