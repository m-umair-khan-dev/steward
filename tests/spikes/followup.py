"""Follow-up checks for spikes S3 (why key injection did not reach the app) and
S5 (which bwrap options work inside a container). Prints JSON."""

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tests" / "e2e"))

import gi  # noqa: E402

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

from harness import App, Home, Session, wait_for  # noqa: E402

out: dict = {}


def bwrap_variants() -> None:
    common = ["--ro-bind", "/usr", "/usr", "--symlink", "usr/lib", "/lib", "--symlink",
              "usr/lib64", "/lib64", "--symlink", "usr/bin", "/bin", "--dev", "/dev",
              "--die-with-parent"]
    variants = {
        "unshare_all_no_proc": ["--unshare-all"],
        "unshare_all_bind_proc": ["--unshare-all", "--ro-bind", "/proc", "/proc"],
        "unshare_net_user_ipc_no_pid": ["--unshare-user", "--unshare-net", "--unshare-ipc",
                                        "--unshare-uts", "--unshare-cgroup-try"],
    }
    results = {}
    for name, extra in variants.items():
        run = subprocess.run(["bwrap", *common, *extra, "true"], capture_output=True, text=True)
        net = subprocess.run(["bwrap", *common, *extra, "python3", "-c",
                              "import socket; socket.create_connection(('1.1.1.1', 53), timeout=2)"],
                             capture_output=True, text=True)
        results[name] = {"runs": run.returncode == 0, "stderr": run.stderr.strip()[-200:],
                         "network_blocked": net.returncode != 0 and "bwrap:" not in net.stderr}
    out["S5_bwrap_variants"] = results


def key_injection(work: Path) -> None:
    subprocess.run([sys.executable, "-I", str(REPO / "docker/scripts/make-fixture-home"),
                    str(work / "home")], check=True, capture_output=True)
    home = Home(work / "home")
    os.environ["STEWARD_COMPOSITOR"] = "mutter"
    session = Session(work / "session", home.path)
    session.start()
    app = App(session, home)
    result: dict = {}
    try:
        app.launch()
        result["window_is_active_after_launch"] = app.state()["active_window"]["is_active"]
        bus = app.bus
        dest, root = "org.gnome.Mutter.RemoteDesktop", "/org/gnome/Mutter/RemoteDesktop"
        (path,) = bus.call_sync(dest, root, "org.gnome.Mutter.RemoteDesktop", "CreateSession", None,
                                GLib.VariantType("(o)"), Gio.DBusCallFlags.NONE, 5000, None).unpack()
        iface = "org.gnome.Mutter.RemoteDesktop.Session"
        bus.call_sync(dest, path, iface, "Start", None, None, Gio.DBusCallFlags.NONE, 5000, None)
        # The first input event makes mutter focus the window; send a harmless
        # Shift tap, then wait for focus before the real shortcut.
        for pressed in (True, False):
            bus.call_sync(dest, path, iface, "NotifyKeyboardKeycode",
                          GLib.Variant("(ub)", (42, pressed)), None, Gio.DBusCallFlags.NONE,
                          5000, None)
        wait_for(lambda: app.state()["active_window"]["is_active"], "window focus", timeout=10)
        result["focused_after_first_input"] = True
        # Evdev keycodes this time: KEY_LEFTCTRL=29, KEY_Q=16.
        for code, pressed in ((29, True), (16, True), (16, False), (29, False)):
            bus.call_sync(dest, path, iface, "NotifyKeyboardKeycode",
                          GLib.Variant("(ub)", (code, pressed)), None, Gio.DBusCallFlags.NONE,
                          5000, None)
        try:
            wait_for(lambda: not app.running(), "app to quit", timeout=5)
            result["keycode_injection"] = "app quit"
        except AssertionError:
            result["keycode_injection"] = "no reaction"
            result["window_is_active_after_keys"] = app.state()["active_window"]["is_active"]
        log = (session.logs / "compositor.log").read_text(errors="replace")
        result["compositor_log_tail"] = log.strip().splitlines()[-8:]
    finally:
        app.stop()
        session.stop()
    out["S3_key_injection"] = result


with tempfile.TemporaryDirectory() as tmp:
    bwrap_variants()
    try:
        key_injection(Path(tmp))
    except Exception as error:  # noqa: BLE001
        out["S3_key_injection_error"] = repr(error)
print(json.dumps(out, indent=2))
