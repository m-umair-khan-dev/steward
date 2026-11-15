"""A private D-Bus session and headless Wayland compositor for one test."""

import os
import signal
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
START_COMPOSITOR = REPO / "docker" / "scripts" / "start-compositor"


class Session:
    def __init__(self, root: Path, home: Path) -> None:
        self.root = root
        self.home = home
        self.runtime = root / "runtime"
        self.logs = root / "logs"
        self.bus_address = ""
        self.wayland_display = ""
        self._dbus: subprocess.Popen | None = None
        self._compositor_pid: int | None = None

    def base_env(self) -> dict[str, str]:
        """Environment for every process in the session. Nothing from the
        outer environment leaks in except PATH, so tests are reproducible."""
        env = {
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
            "HOME": str(self.home),
            "XDG_RUNTIME_DIR": str(self.runtime),
            "LANG": "C.UTF-8",
            "LC_ALL": "C.UTF-8",
            "NO_AT_BRIDGE": "0",
        }
        for key in ("CAIRN_COMPOSITOR",):
            if os.environ.get(key):
                env[key] = os.environ[key]
        if self.bus_address:
            env["DBUS_SESSION_BUS_ADDRESS"] = self.bus_address
        if self.wayland_display:
            env["WAYLAND_DISPLAY"] = self.wayland_display
            env["GDK_BACKEND"] = "wayland"
        return env

    def start(self) -> None:
        self.runtime.mkdir(parents=True)
        os.chmod(self.runtime, 0o700)
        self.logs.mkdir(parents=True)

        self._dbus = subprocess.Popen(
            ["dbus-daemon", "--session", "--nofork", "--nopidfile",
             f"--address=unix:path={self.runtime}/bus", "--print-address=1"],
            stdout=subprocess.PIPE, stderr=open(self.logs / "dbus.log", "wb"),
            env=self.base_env(), text=True,
        )
        assert self._dbus.stdout is not None
        self.bus_address = self._dbus.stdout.readline().strip()
        if not self.bus_address:
            raise RuntimeError("dbus-daemon did not report an address; see dbus.log")

        # The compositor keeps its caches (e.g. Mesa's shader cache) outside the
        # test home, so checks that the home is unchanged only see the app.
        compositor_cache = self.root / "compositor-cache"
        compositor_cache.mkdir()
        env = self.base_env() | {
            "COMPOSITOR_LOG": str(self.logs / "compositor.log"),
            "XDG_CACHE_HOME": str(compositor_cache),
            "MESA_SHADER_CACHE_DIR": str(compositor_cache / "mesa"),
        }
        result = subprocess.run([str(START_COMPOSITOR)], env=env, capture_output=True, text=True,
                                timeout=30)
        if result.returncode != 0:
            raise RuntimeError(f"compositor did not start: {result.stderr.strip()}")
        pid, socket = result.stdout.split()
        self._compositor_pid = int(pid)
        self.wayland_display = socket

    def stop(self) -> None:
        if self._compositor_pid:
            try:
                os.kill(self._compositor_pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            self._compositor_pid = None
        if self._dbus and self._dbus.poll() is None:
            self._dbus.terminate()
            try:
                self._dbus.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self._dbus.kill()
