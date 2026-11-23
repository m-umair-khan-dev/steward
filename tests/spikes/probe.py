"""Phase 0 spike probe: answers the platform questions in DESIGN.md §10 Phase 0
with evidence from the current image. Rerun on every new Ubuntu release:

    docker compose -f compose.desktop.yaml run --rm e2e docker/scripts/run-e2e python3 tests/spikes/probe.py

Prints one JSON report. Each check records what it ran and what happened.
"""

import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tests" / "e2e"))

import gi  # noqa: E402

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

from harness import App, Home, Session, wait_for  # noqa: E402

REPORT: dict = {}


def sh(*cmd: str, **kwargs) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, capture_output=True, text=True, **kwargs)


def record(name: str, ok: bool, **details) -> None:
    REPORT[name] = {"ok": ok, **details}


# --- S1: library versions and baseline widgets ---------------------------------

def s1_versions() -> None:
    modules = ["gtk4", "libadwaita-1", "gtksourceview-5", "webkitgtk-6.0", "poppler-glib",
               "glib-2.0", "gstreamer-1.0"]
    versions = {m: sh("pkg-config", "--modversion", m).stdout.strip() or "missing"
                for m in modules}
    for tool in ("blueprint-compiler", "mutter", "weston", "bwrap"):
        out = sh(tool, "--version")
        versions[tool] = (out.stdout or out.stderr).strip().splitlines()[0] if out.returncode == 0 \
            else "missing"
    versions["os"] = Path("/etc/os-release").read_text().split('PRETTY_NAME="')[1].split('"')[0]

    gi.require_version("Gtk", "4.0")
    gi.require_version("Adw", "1")
    from gi.repository import Adw, Gtk  # noqa: PLC0415
    wanted = {
        "Adw": ["ApplicationWindow", "OverlaySplitView", "NavigationView", "NavigationSplitView",
                "ToolbarView", "Dialog", "AlertDialog", "AboutDialog", "PreferencesDialog",
                "ToastOverlay", "StatusPage", "Banner", "BottomSheet", "Spinner", "WrapBox"],
        "Gtk": ["GridView", "ColumnView", "SectionModel", "ListView", "FileDialog",
                "WidgetPaintable", "GraphicsOffload"],
    }
    namespaces = {"Adw": Adw, "Gtk": Gtk}
    present = {ns: {w: hasattr(namespaces[ns], w) for w in names} for ns, names in wanted.items()}
    record("S1_versions", True, versions=versions, widgets=present)


# --- S2: headless compositors ---------------------------------------------------

def s2_compositors() -> None:
    results = {}
    for kind in ("mutter", "weston"):
        with tempfile.TemporaryDirectory() as runtime:
            os.chmod(runtime, 0o700)
            env = dict(os.environ, XDG_RUNTIME_DIR=runtime, STEWARD_COMPOSITOR=kind,
                       COMPOSITOR_LOG=f"{runtime}/log")
            started = time.monotonic()
            out = sh("dbus-run-session", "--", "bash", "-c",
                     f"read -r pid sock < <({REPO}/docker/scripts/start-compositor) && "
                     "echo started $sock && kill $pid", env=env, timeout=60)
            results[kind] = {"ok": "started" in out.stdout,
                             "seconds": round(time.monotonic() - started, 2),
                             "stderr": out.stderr.strip()[-400:]}
    record("S2_compositors", all(r["ok"] for r in results.values()), compositors=results)


# --- S3: accessibility tree and input injection ----------------------------------

def s3_a11y_and_input(work: Path) -> None:
    subprocess.run([sys.executable, "-I", str(REPO / "docker/scripts/make-fixture-home"),
                    str(work / "home")], check=True, capture_output=True)
    home = Home(work / "home")
    os.environ["STEWARD_COMPOSITOR"] = "mutter"
    session = Session(work / "session", home.path)
    session.start()
    app = App(session, home)
    details: dict = {}
    try:
        app.launch()
        names = app.a11y_names()
        details["a11y_names_sample"] = sorted(names)[:12]
        details["a11y_tree_visible"] = "Main toolbar" in names

        # Mutter's remote-desktop interface: create a session, start it and press
        # Ctrl+Q. If the app quits, real key input can be injected.
        bus = app.bus
        try:
            reply = bus.call_sync("org.gnome.Mutter.RemoteDesktop", "/org/gnome/Mutter/RemoteDesktop",
                                  "org.gnome.Mutter.RemoteDesktop", "CreateSession", None,
                                  GLib.VariantType("(o)"), Gio.DBusCallFlags.NONE, 5000, None)
            (path,) = reply.unpack()
            iface = "org.gnome.Mutter.RemoteDesktop.Session"
            bus.call_sync("org.gnome.Mutter.RemoteDesktop", path, iface, "Start", None, None,
                          Gio.DBusCallFlags.NONE, 5000, None)
            ctrl, q = 0xFFE3, 0x0071  # XK_Control_L, XK_q
            for keysym, pressed in ((ctrl, True), (q, True), (q, False), (ctrl, False)):
                bus.call_sync("org.gnome.Mutter.RemoteDesktop", path, iface,
                              "NotifyKeyboardKeysym", GLib.Variant("(ub)", (keysym, pressed)),
                              None, Gio.DBusCallFlags.NONE, 5000, None)
            try:
                wait_for(lambda: not app.running(), "the app to quit after Ctrl+Q", timeout=5)
                details["key_injection"] = "works: Ctrl+Q through RemoteDesktop quit the app"
                details["key_injection_ok"] = True
            except AssertionError as error:
                details["key_injection"] = f"keys sent but app did not react: {error}"
                details["key_injection_ok"] = False
        except GLib.Error as error:
            details["key_injection"] = f"RemoteDesktop unavailable: {error.message}"
            details["key_injection_ok"] = False
    finally:
        app.stop()
        session.stop()
    record("S3_a11y_and_input", details.get("a11y_tree_visible", False), **details)


# --- S5: bubblewrap ---------------------------------------------------------------

def s5_bwrap() -> None:
    base = ["bwrap", "--ro-bind", "/usr", "/usr", "--symlink", "usr/lib", "/lib",
            "--symlink", "usr/lib64", "/lib64", "--symlink", "usr/bin", "/bin",
            "--proc", "/proc", "--dev", "/dev", "--unshare-all", "--die-with-parent"]
    plain = sh(*base, "true")
    no_net = sh(*base, "python3", "-c",
                "import socket; socket.create_connection(('1.1.1.1', 53), timeout=2)")
    record("S5_bwrap", plain.returncode == 0,
           sandbox_runs=plain.returncode == 0, sandbox_stderr=plain.stderr.strip()[-300:],
           network_blocked=no_net.returncode != 0,
           network_probe_stderr=no_net.stderr.strip().splitlines()[-1:] if no_net.stderr else [])


# --- S6: Trash and file monitors on the test home's filesystem ----------------------

def s6_trash_and_monitors(work: Path) -> None:
    home = work / "trash-home"
    home.mkdir()
    fs_type = sh("stat", "-f", "-c", "%T", str(home)).stdout.strip()
    victim = home / "Downloads" / "victim.txt"
    victim.parent.mkdir()
    victim.write_text("x")
    env = dict(os.environ, HOME=str(home), XDG_DATA_HOME=str(home / ".local/share"))
    trash = sh("gio", "trash", str(victim), env=env)
    trashed = (home / ".local/share/Trash/files/victim.txt").exists()

    events: list[str] = []
    loop = GLib.MainLoop()
    monitor = Gio.File.new_for_path(str(home / "Downloads")).monitor_directory(
        Gio.FileMonitorFlags.WATCH_MOVES, None)
    monitor.connect("changed", lambda _m, f, _o, ev: (events.append(f"{ev.value_nick}:{f.get_basename()}"),
                                                       loop.quit()))
    GLib.timeout_add(100, lambda: (home / "Downloads" / "new.txt").write_text("y") and False)
    GLib.timeout_add(3000, loop.quit)
    loop.run()
    record("S6_trash_and_monitors", trashed and bool(events), filesystem=fs_type,
           gio_trash_exit=trash.returncode, gio_trash_stderr=trash.stderr.strip(),
           trashed_into_home_trash=trashed, monitor_events=events[:5])


def main() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        for check in (s1_versions, s2_compositors, lambda: s3_a11y_and_input(work), s5_bwrap,
                      lambda: s6_trash_and_monitors(work)):
            try:
                check()
            except Exception as error:  # noqa: BLE001 - record and carry on
                name = getattr(check, "__name__", "check")
                record(f"error_{name}", False, error=repr(error))
    print(json.dumps(REPORT, indent=2))


if __name__ == "__main__":
    main()
