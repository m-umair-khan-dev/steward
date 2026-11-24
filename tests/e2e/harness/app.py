"""The real Steward binary, started inside a test session and driven like a user."""

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

from .keys import Keyboard  # noqa: E402
from .session import Session  # noqa: E402
from .wait import wait_for  # noqa: E402

APP_ID = "io.github.m_umair_khan_dev.steward"
APP_PATH = "/io/github/m_umair_khan_dev/steward"
TEST_IFACE = "io.github.m_umair_khan_dev.steward.Test1"
ATSPI_TOOL = Path(__file__).with_name("atspi_tool.py")
DEFAULT_BIN = "/var/cache/cargo-target/e2e/debug/steward"  # built by docker/scripts/run-e2e


class App:
    def __init__(self, session: Session, home, binary: str | None = None,
                 extra_env: dict[str, str] | None = None) -> None:
        self.session = session
        self.home = home
        self.binary = binary or os.environ.get("STEWARD_BIN", DEFAULT_BIN)
        self.extra_env = extra_env or {}
        self.log_path = session.logs / "app.log"
        self.process: subprocess.Popen | None = None
        self._bus: Gio.DBusConnection | None = None
        self._keyboard: Keyboard | None = None
        self._pointer_ready = False

    # -- lifecycle --------------------------------------------------------

    def env(self) -> dict[str, str]:
        return self.session.base_env() | {
            "STEWARD_E2E": "1",
            # No first-run welcome unless a test asks (E6-01 sets it empty).
            "STEWARD_NO_WELCOME": "1",
            "G_DEBUG": "fatal-criticals",
            "RUST_BACKTRACE": "1",
            "QT_QPA_PLATFORM": "wayland",
            # GPU shader caches belong to the machine, not the fixture home
            # (the index would count them, E3-01).
            "MESA_SHADER_CACHE_DIR": str(self.session.root / "mesa-app"),
            # Qt's shader cache too (Qt 6.4 writes it at start-up, 24.04).
            "QT_DISABLE_SHADER_DISK_CACHE": "1",
            "QT_LINUX_ACCESSIBILITY_ALWAYS_ON": "1",
            "QT_WAYLAND_DISABLE_WINDOWDECORATION": "1",
        } | self.extra_env

    def launch(self, wait: bool = True, timeout: float = 20.0) -> "App":
        log = open(self.log_path, "ab")
        self.process = subprocess.Popen([self.binary], env=self.env(), stdout=log,
                                        stderr=subprocess.STDOUT)
        if wait:
            wait_for(self._name_owned, "the app to own its D-Bus name", timeout)
            wait_for(self._window_ready, "the main window to be shown", timeout)
        return self

    def running(self) -> bool:
        return self.process is not None and self.process.poll() is None

    def wait_exit(self, timeout: float) -> int:
        assert self.process is not None
        try:
            return self.process.wait(timeout=timeout)
        except subprocess.TimeoutExpired as error:
            raise AssertionError(f"app still running after {timeout}s") from error

    def stop(self) -> None:
        if not self.running():
            return
        try:
            self.action("app.quit")
            self.process.wait(timeout=5)
        except Exception:  # noqa: BLE001 - fall back to killing it
            self.process.kill()
            self.process.wait(timeout=5)

    def log_text(self) -> str:
        return self.log_path.read_text(errors="replace") if self.log_path.exists() else ""

    # -- D-Bus ------------------------------------------------------------

    @property
    def bus(self) -> Gio.DBusConnection:
        if self._bus is None:
            self._bus = Gio.DBusConnection.new_for_address_sync(
                self.session.bus_address,
                Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT
                | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,
                None, None)
        return self._bus

    def _call(self, path: str, iface: str, method: str, args: GLib.Variant | None,
              reply: str | None, timeout_ms: int = 10_000):
        result = self.bus.call_sync(APP_ID, path, iface, method, args,
                                    GLib.VariantType(reply) if reply else None,
                                    Gio.DBusCallFlags.NONE, timeout_ms, None)
        return result.unpack() if result is not None else None

    def _name_owned(self) -> bool:
        if not self.running():
            raise AssertionError(f"app exited early with {self.process.returncode}:\n"
                                 f"{self.log_text()[-2000:]}")
        reply = self.bus.call_sync(
            "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
            "NameHasOwner", GLib.Variant("(s)", (APP_ID,)), GLib.VariantType("(b)"),
            Gio.DBusCallFlags.NONE, 2000, None)
        return reply.unpack()[0]

    @property
    def installed(self) -> bool:
        """Testing the installed package: a release build without Test1, so
        only public interfaces (D-Bus actions, accessibility) are used."""
        return os.environ.get("STEWARD_INSTALLED") == "1"

    def _window_ready(self) -> bool:
        if self.installed:
            return "Steward" in self.a11y_names()
        window = self.state().get("active_window")
        return bool(window and window["width"] > 0)

    def dismiss_welcome(self) -> None:
        """Release builds show the welcome on a first run: skip it."""
        if "Welcome to Steward" in self.a11y_names():
            self.a11y_click("Skip", role="button", within="Welcome to Steward")
            wait_for(lambda: "Welcome to Steward" not in self.a11y_names(), "the welcome to close", 10)

    def action(self, name: str, parameter: GLib.Variant | None = None) -> None:
        """Activates a GAction the way a menu item or shortcut would.
        `app.<name>` targets the application, `win.<name>` the first window."""
        scope, _, action = name.partition(".")
        path = {"app": APP_PATH, "win": f"{APP_PATH}/window/1"}[scope]
        params = [parameter] if parameter is not None else []
        args = GLib.Variant("(sava{sv})", (action, params, {}))
        try:
            self._call(path, "org.gtk.Actions", "Activate", args, None)
        except GLib.Error:
            # Quitting can close the connection before the reply arrives.
            if action != "quit":
                raise

    def state(self) -> dict:
        (text,) = self._call(f"{APP_PATH}/Test1", TEST_IFACE, "State", None, "(s)")
        return json.loads(text)

    def wait_idle(self, timeout: float = 10.0) -> None:
        (idle,) = self._call(f"{APP_PATH}/Test1", TEST_IFACE, "WaitIdle",
                             GLib.Variant("(u)", (int(timeout * 1000),)), "(b)",
                             timeout_ms=int(timeout * 1000) + 2000)
        assert idle, f"app not idle after {timeout}s"

    # -- the indexer (Phase 3) -----------------------------------------------

    INDEXER = ("io.github.m_umair_khan_dev.steward.Indexer", "/io/github/m_umair_khan_dev/steward/Indexer", "io.github.m_umair_khan_dev.steward.Indexer1")

    def indexer(self, method: str, args: GLib.Variant | None = None, reply: str | None = None,
                timeout_ms: int = 5000):
        name, path, iface = self.INDEXER
        result = self.bus.call_sync(name, path, iface, method, args,
                                    GLib.VariantType(reply) if reply else None,
                                    Gio.DBusCallFlags.NO_AUTO_START, timeout_ms, None)
        return result.unpack() if result is not None else None

    def index_status(self) -> dict:
        return json.loads(self.indexer("Status", reply="(s)")[0])

    def wait_index_idle(self, timeout: float = 30.0) -> dict:
        """Waits for the indexer to start and to have nothing left to do."""
        wait_for(lambda: self.index_status(), "the indexer to run", timeout)
        (idle,) = self.indexer("WaitIdle", GLib.Variant("(u)", (int(timeout * 1000),)), "(b)",
                               timeout_ms=int(timeout * 1000) + 2000)
        assert idle, f"indexer not idle after {timeout}s"
        status = self.index_status()
        wait_for(lambda: self.state()["index"]["running"], "the app to see the indexer", 10)
        return status

    def screenshot(self, path: Path | str) -> Path:
        path = Path(path)
        path.parent.mkdir(parents=True, exist_ok=True)
        self._call(f"{APP_PATH}/Test1", TEST_IFACE, "Screenshot",
                   GLib.Variant("(s)", (str(path),)), "(b)")
        return path

    def wait_for(self, condition, what: str, timeout: float = 5.0):
        return wait_for(condition, what, timeout)

    def press(self, combo: str) -> None:
        """Presses real keys, e.g. app.press("ctrl+q"), and waits until the
        window had keyboard focus first. Needs the mutter compositor."""
        if self._keyboard is None:
            self._keyboard = Keyboard(self.bus)
            self._keyboard.start()
            wait_for(lambda: self.state()["active_window"]["is_active"],
                     "the window to get keyboard focus", timeout=10)
            # The compositor can drop the very first key event while the
            # client is still taking its keymap; a lone Shift does nothing.
            self._keyboard.press("shift")
            self.wait_idle()
        self._keyboard.press(combo)

    def _input(self) -> Keyboard:
        if self._keyboard is None:
            self.press("shift")  # opens the session and waits for focus
        return self._keyboard

    def click_at(self, x: float, y: float, button: str = "left") -> None:
        """A real pointer click at window coordinates. The window fills the
        compositor's 1280x800 screen (Main.qml), so they are screen coordinates.
        The pointer moves first and the app is let idle (at least one frame),
        so the compositor has given it pointer focus before the button goes
        down; otherwise the first click of a session is lost."""
        device = self._input()
        if not self._pointer_ready:
            # Mutter drops the first pointer motion of a remote-desktop
            # session, which would leave the first click wherever the pointer
            # started: spend it on a move that goes nowhere in particular.
            device.move_to(0, 0)
            self.wait_idle()
            self._pointer_ready = True
        device.move_to(x, y)
        self.wait_idle()
        device.button(button, True)
        device.button(button, False)

    def find_node(self, name: str, role: str | None = None, within: str | None = None,
                  tree: dict | None = None) -> dict | None:
        """The first showing node with this name (and role), optionally inside
        the subtree of a showing node named `within`."""
        def walk(node, pick):
            if node["showing"] and pick(node):
                return node
            for child in node["children"]:
                found = walk(child, pick)
                if found is not None:
                    return found
            return None
        root = tree or self.a11y_tree()
        if within:
            root = walk(root, lambda n: n["name"] == within)
            if root is None:
                return None
        aliases = {"push button": "button", "grouping": "panel"}
        return walk(root, lambda n: n["name"] == name
                    and (role is None or aliases.get(n["role"], n["role"]) == role)
                    and bool(n.get("extents")) and n["extents"][2] > 0)

    def find_node_role(self, roles: tuple[str, ...]) -> dict | None:
        """The first showing node with one of these roles (e.g. an open menu)."""
        stack = [self.a11y_tree()]
        while stack:
            node = stack.pop()
            if node["showing"] and node["role"] in roles:
                return node
            stack.extend(node["children"])
        return None

    def mouse_click(self, name: str, role: str | None = None, within: str | None = None,
                    button: str = "left", timeout: float = 10.0) -> None:
        """Clicks the centre of a control with the real pointer, as a person would."""
        node = wait_for(lambda: self.find_node(name, role, within),
                        f"a showing {name!r} to click", timeout)
        x, y, w, h = node["extents"]
        self.click_at(x + w / 2, y + h / 2, button)

    def type_text(self, text: str) -> None:
        """Types text with real key presses (US layout characters only)."""
        from .keys import combo_for
        for char in text:
            self.press(combo_for(char))

    # -- folder helpers -----------------------------------------------------

    def folder(self) -> dict:
        return self.state()["folder"]

    def open_folder(self, path, timeout: float = 15.0) -> dict:
        """Opens a folder through win.go-to and waits until it has loaded."""
        path = str(path)
        self.action("win.go-to", GLib.Variant("s", path))

        def loaded():
            folder = self.folder()
            return folder if folder["path"] == path and folder["state"] != "loading" else None

        return wait_for(loaded, f"{path} to load", timeout)

    def select(self, *names: str) -> None:
        """Selects items in the folder view by name (Test1.Select)."""
        self._call(f"{APP_PATH}/Test1", TEST_IFACE, "Select", GLib.Variant("(as)", (list(names),)),
                   "(b)")

    # -- accessibility tree -------------------------------------------------
    # AT-SPI keeps one bus connection per process, so each query runs in a short
    # helper process with this session's environment.

    def _atspi(self, *args: str, timeout: float = 15.0) -> subprocess.CompletedProcess:
        assert self.process is not None
        return subprocess.run(
            [sys.executable, "-I", str(ATSPI_TOOL), str(self.process.pid), *args],
            env=self.session.base_env(), capture_output=True, text=True, timeout=timeout)

    def a11y_tree(self) -> dict:
        # Dumping a whole screen walks every accessible over D-Bus; on a busy
        # machine (the full suite) that can take longer than one click.
        result = self._atspi("dump", timeout=60.0)
        if result.returncode != 0:
            raise AssertionError(f"accessibility dump failed: {result.stderr.strip()}")
        return json.loads(result.stdout)

    def a11y_names(self, showing_only: bool = True) -> set[str]:
        names: set[str] = set()

        def walk(node: dict) -> None:
            if node["name"] and (node["showing"] or not showing_only):
                names.add(node["name"])
            for child in node["children"]:
                walk(child)

        walk(self.a11y_tree())
        return names

    def a11y_click(self, name: str, role: str | None = None, within: str | None = None) -> None:
        """Performs a node's default action, as a screen reader user would.
        `role` is an AT-SPI role name ("button", "toggle button", ...).
        `within` limits the search to the subtree of a node with that name."""
        result = self._atspi("click", name, role or "", within or "")
        if result.returncode != 0:
            raise AssertionError(f"could not click {name!r}: {result.stderr.strip()}")

    def a11y_focus(self, name: str, role: str | None = None, within: str | None = None) -> None:
        """Gives a node keyboard focus through the accessibility tree."""
        result = self._atspi("focus", name, role or "", within or "")
        if result.returncode != 0:
            raise AssertionError(f"could not focus {name!r}: {result.stderr.strip()}")

    # -- failure artifacts ------------------------------------------------

    def collect_artifacts(self, dest: Path) -> None:
        dest.mkdir(parents=True, exist_ok=True)
        for log in self.session.logs.glob("*.log"):
            shutil.copy(log, dest / log.name)
        if not self.running():
            return
        for name, fetch in (("state.json", lambda: json.dumps(self.state(), indent=2)),
                            ("a11y.json", lambda: json.dumps(self.a11y_tree(), indent=1))):
            try:
                (dest / name).write_text(fetch())
            except Exception as error:  # noqa: BLE001 - best effort
                (dest / f"{name}.error").write_text(repr(error))
        try:
            self.screenshot(dest / "screenshot.png")
        except Exception as error:  # noqa: BLE001 - best effort
            (dest / "screenshot.error").write_text(repr(error))
