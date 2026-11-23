"""Checks for the Qt/QML proof (spikes/qt-proof), under headless mutter:

1. steward-core's folder listing (GIO on the GLib loop Qt runs) reaches QML;
2. the accessibility tree is visible to the harness;
3. real key presses drive it (Ctrl+K, Delete, Ctrl+Q);
4. a scripted demo records frames to artifacts/qt-proof/frames.

    spikes/qt-proof/run python3 /workspace/tests/spikes/qt_proof_check.py
"""

import json
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tests" / "e2e"))

import gi  # noqa: E402

gi.require_version("Gio", "2.0")
from gi.repository import Gio  # noqa: E402

from harness import Home, Session, wait_for  # noqa: E402
from harness.keys import Keyboard  # noqa: E402

BIN = "/var/cache/cargo-target/debug/steward-qt-proof"
ATSPI = REPO / "tests" / "e2e" / "harness" / "atspi_tool.py"
OUT = REPO / "artifacts" / "qt-proof"
OUT.mkdir(parents=True, exist_ok=True)
report: dict = {}


def names_of(session, pid) -> set[str]:
    result = subprocess.run([sys.executable, "-I", str(ATSPI), str(pid), "dump"],
                            env=session.base_env(), capture_output=True, text=True, timeout=20)
    if result.returncode != 0:
        raise AssertionError(result.stderr)
    found: set[str] = set()

    def walk(node):
        if node["name"]:
            found.add(node["name"])
        for child in node["children"]:
            walk(child)

    walk(json.loads(result.stdout))
    return found


def window_active(session, pid) -> bool:
    result = subprocess.run([sys.executable, "-I", str(ATSPI), str(pid), "dump"],
                            env=session.base_env(), capture_output=True, text=True, timeout=20)
    tree = json.loads(result.stdout)
    return any(child["active"] for child in tree["children"])


def launch(session, home, extra=None):
    env = session.base_env() | {
        "QT_QPA_PLATFORM": "wayland",
        "QT_LINUX_ACCESSIBILITY_ALWAYS_ON": "1",
        "STEWARD_PROOF_FOLDER": str(home / "Downloads"),
    } | (extra or {})
    log = open(session.logs / "proof.log", "ab")
    return subprocess.Popen([BIN], env=env, stdout=log, stderr=subprocess.STDOUT)


with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
    work = Path(tmp)
    subprocess.run([sys.executable, "-I", str(REPO / "docker/scripts/make-fixture-home"),
                    str(work / "home")], check=True, capture_output=True)
    home = Home(work / "home")
    session = Session(work / "session", home.path)
    session.start()
    try:
        # 1-3: interactive checks.
        proc = launch(session, home)
        folder_names = {"archive", "Receipts", "Wallpapers"}
        names = wait_for(lambda: (n := names_of(session, proc.pid)) and folder_names <= n and n,
                         "the Rust listing to reach QML", timeout=30)
        report["listing_reached_qml"] = True
        report["a11y_sample"] = sorted(n for n in names if len(n) < 40)[:25]

        bus = Gio.DBusConnection.new_for_address_sync(
            session.bus_address,
            Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,
            None, None)
        keys = Keyboard(bus)
        keys.start()
        wait_for(lambda: window_active(session, proc.pid), "the window to get focus", timeout=10)
        def attempt(label, combo, condition):
            keys.press(combo)
            try:
                report[label] = bool(wait_for(condition, label, timeout=8))
            except AssertionError:
                report[label] = False
                report[label + "_names"] = sorted(n for n in names_of(session, proc.pid) if len(n) < 50)

        attempt("ctrl_k_groups_by_kind", "ctrl+k", lambda: "Grouped by kind" in names_of(session, proc.pid))
        attempt("delete_shows_toast", "delete", lambda: any(
            n.startswith("Moved “npm download cache”") for n in names_of(session, proc.pid)))
        keys.press("ctrl+q")
        try:
            report["ctrl_q_exit_code"] = proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            report["ctrl_q_exit_code"] = "still running"
            proc.kill()

        # 4: scripted demo with frame capture.
        frames = OUT / "frames"
        frames.mkdir(exist_ok=True)
        for old in frames.glob("*.png"):
            old.unlink()
        proc = launch(session, home, {"STEWARD_CAPTURE_DIR": str(frames)})
        report["capture_exit_code"] = proc.wait(timeout=120)
        report["frames"] = len(list(frames.glob("*.png")))
    finally:
        log = session.logs / "proof.log"
        report["log_tail"] = log.read_text(errors="replace").splitlines()[-12:] if log.exists() else []
        session.stop()

(OUT / "check.json").write_text(json.dumps(report, indent=1, ensure_ascii=False))
print(json.dumps(report, indent=1, ensure_ascii=False))
