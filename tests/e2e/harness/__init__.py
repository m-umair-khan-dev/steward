"""End-to-end test harness for Cairn (DESIGN.md §10.2).

- `Session`: a private D-Bus session and headless Wayland compositor.
- `App`: the real app binary, started in a session and driven over D-Bus
  (GActions, the Test1 interface) and the accessibility tree.
- `Home`: a fresh copy of the fixture home, with snapshots for before/after checks.
"""

from .app import App
from .home import Home
from .session import Session
from .wait import WaitTimeout, wait_for

__all__ = ["App", "Home", "Session", "WaitTimeout", "wait_for"]
