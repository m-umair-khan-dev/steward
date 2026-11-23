"""A per-test copy of the fixture home."""

import hashlib
import json
import os
import stat
from pathlib import Path

MARKER = ".steward-fixture"
BIG = 16 * 1024 * 1024

# Folders the app may write to as part of normal running (its own data, caches
# and settings). Read-only checks ignore these; everything else must not change.
APP_OWNED = (
    ".local/share/steward",
    ".cache/steward",
    ".config/steward",
    ".config/glib-2.0",
    ".config/dconf",
    ".cache/thumbnails",
    ".cache/mesa_shader_cache",
    ".cache/mesa_shader_cache_db",
    ".cache/qtshadercache",
    ".local/share/recently-used.xbel",
)


class Home:
    def __init__(self, path: Path) -> None:
        self.path = path
        self._manifest: dict | None = None

    def __truediv__(self, other: str) -> Path:
        return self.path / other

    def __fspath__(self) -> str:
        return str(self.path)

    @property
    def manifest(self) -> dict:
        """The fixture manifest (read once, before any test changes the marker)."""
        if self._manifest is None:
            self._manifest = json.loads((self.path / MARKER).read_text())
        return self._manifest

    def snapshot(self, sub: str = "", ignore_app_owned: bool = False) -> dict[str, tuple]:
        """Paths under `sub` mapped to (type, mode, size, mtime_ns, content hash).

        Two snapshots compare equal only if nothing was added, removed, moved,
        changed or re-timed. Files over 16 MiB are hashed by size and first MiB."""
        root = self.path / sub
        result: dict[str, tuple] = {}
        for dirpath, dirnames, filenames in os.walk(root):
            dirnames.sort()
            for name in sorted(dirnames + filenames):
                full = Path(dirpath) / name
                rel = full.relative_to(self.path).as_posix()
                if ignore_app_owned and rel.startswith(APP_OWNED):
                    continue
                st = full.lstat()
                if stat.S_ISDIR(st.st_mode):
                    result[rel] = ("d", st.st_mode & 0o7777)
                    continue
                digest = ""
                if stat.S_ISREG(st.st_mode):
                    with open(full, "rb") as handle:
                        digest = hashlib.sha256(handle.read(1 << 20 if st.st_size > BIG
                                                            else st.st_size)).hexdigest()
                kind = "l" if stat.S_ISLNK(st.st_mode) else "f"
                result[rel] = (kind, st.st_mode & 0o7777, st.st_size, st.st_mtime_ns, digest)
        return result
