"""Clicks every control on a screen with the real pointer (Phase 10).

A screen is a setup function that puts a freshly started app in some state
(a folder, a tool page, a dialog). The sweep reads the screen's controls
from the accessibility tree and clicks each one at its centre. After every
click it checks that the app is still running, goes idle, logged no QML
error, and, inside a dialog, that a control which only changes a setting
(tab, switch, radio button, drop-down, segmented button) left the dialog
open. A click that changes the screen is undone (Escape and the setup
again, or a restart) before the next control is clicked.
"""

import json
import re
from dataclasses import dataclass, field
from pathlib import Path

from .wait import WaitTimeout, wait_for

CLICKABLE = {"button", "push button", "toggle button", "check box", "radio button",
             "page tab", "combo box", "menu item", "check menu item", "radio menu item", "link"}
# Clicking these changes a setting or a page; it must not close the dialog.
SETTING_ROLES = {"check box", "radio button", "page tab", "combo box", "toggle button"}
# Clicking these ends the app on purpose.
SKIP = {"Quit"}
# Lines in the app's log that mean the UI broke.
PROBLEM = re.compile(r"qrc:/\S+: (?!.*Binding loop)|TypeError|ReferenceError|panicked|"
                     r"CRITICAL|Segmentation fault|Aborted")
# State that changes when a click moves to another screen or layout.
SIGNATURE_KEYS = ("dialog", "folder.path", "folder.page", "folder.view_mode", "tools.page",
                  "search.open", "peek.open", "pane.open", "workspace.open",
                  "preview.visible", "sidebar.expanded")


def _get(state: dict, dotted: str):
    value = state
    for part in dotted.split("."):
        value = value.get(part) if isinstance(value, dict) else None
    return value


def signature(app) -> str:
    state = app.state()
    return json.dumps({key: _get(state, key) for key in SIGNATURE_KEYS}, sort_keys=True)


def _scope(tree: dict, name: str | None) -> dict | None:
    if name is None:
        return tree
    # "@menu" is whichever menu is open (Qt's menus have no name).
    menu = name == "@menu"
    stack = [tree]
    while stack:
        node = stack.pop()
        if not node["showing"]:
            stack.extend(node["children"])
            continue
        if menu and node["role"] in ("menu", "popup menu"):
            return node
        if not menu and node["name"] == name and node["role"] in ("dialog", "window"):
            return node
        stack.extend(node["children"])
    return None


def controls(tree: dict, within: str | None) -> list[dict]:
    """Showing, enabled, named controls with a size, in tree order. Each is
    {role, name, nth, setting, extents}; nth tells apart controls that share
    a role and name."""
    root = _scope(tree, within)
    found: list[dict] = []
    seen: dict[tuple[str, str], int] = {}

    def walk(node: dict, parent_role: str) -> None:
        role = node["role"]
        ext = node.get("extents")
        if (role in CLICKABLE and node["showing"] and node["enabled"] and node["name"]
                and ext and ext[2] > 1 and ext[3] > 1 and node["name"] not in SKIP):
            key = (role, node["name"])
            nth = seen.get(key, 0)
            seen[key] = nth + 1
            found.append({
                "role": role, "name": node["name"], "nth": nth, "extents": ext,
                # A button inside a grouping is one choice of a segmented control.
                "setting": role in SETTING_ROLES or (role in ("button", "push button")
                                                     and parent_role in ("panel", "grouping")),
            })
        for child in node["children"]:
            walk(child, role)

    if root is not None:
        walk(root, "")
    return found


def locate(tree: dict, within: str | None, control: dict) -> dict | None:
    for candidate in controls(tree, within):
        if (candidate["role"], candidate["name"], candidate["nth"]) == (
                control["role"], control["name"], control["nth"]):
            return candidate
    return None


@dataclass
class Result:
    screen: str
    clicked: int = 0
    failures: list[dict] = field(default_factory=list)
    changed: list[str] = field(default_factory=list)
    missing: list[str] = field(default_factory=list)

    def fail(self, control: dict, kind: str, detail: str) -> None:
        self.failures.append({"control": f"{control['role']} {control['name']!r}",
                              "kind": kind, "detail": detail})


class Sweep:
    """Runs one screen. `start()` returns a new, running app; `setup(app)`
    shows the screen; `within` names the dialog or menu whose controls are
    clicked (None: the whole window)."""

    def __init__(self, name: str, start, setup, within: str | None, report_dir: Path) -> None:
        self.name = name
        self.start = start
        self.setup = setup
        self.within = within
        self.report_dir = report_dir
        self.result = Result(name)
        self.app = None
        self.base = ""
        self.log_offset = 0

    # -- the screen -------------------------------------------------------

    def _show(self, fresh: bool) -> dict:
        if fresh:
            if self.app is not None:
                self.app.stop()
            self.app = self.start()
        self.setup(self.app)
        self.app.wait_idle()
        self.base = signature(self.app)
        self.log_offset = len(self.app.log_text())
        return self.app.a11y_tree()

    def _restore(self) -> dict | None:
        """Back to the screen after a click changed it; None if it was
        already there. Escape first (closes menus and dialogs), then the
        setup again, then a restart."""
        if self.app.running():
            try:
                if signature(self.app) == self.base and self._menu_ok():
                    return None
                for _ in range(2):
                    self.app.press("escape")
                    self.app.wait_idle()
                return self._show(fresh=False) if self._settled() else self._show(fresh=True)
            except Exception:  # noqa: BLE001 - restart below
                pass
        return self._show(fresh=True)

    def _settled(self) -> bool:
        try:
            self.setup(self.app)
            self.app.wait_idle()
            return signature(self.app) == self.base
        except Exception:  # noqa: BLE001
            return False

    def _menu_ok(self) -> bool:
        """A menu screen is only intact while the menu is still showing."""
        if self.within is None:
            return True
        return _scope(self.app.a11y_tree(), self.within) is not None

    def _new_problems(self) -> list[str]:
        text = self.app.log_text()
        fresh, self.log_offset = text[self.log_offset:], len(text)
        return [line for line in fresh.splitlines() if PROBLEM.search(line)]

    # -- the run ----------------------------------------------------------

    def run(self) -> Result:
        tree = self._show(fresh=True)
        todo = controls(tree, self.within)
        if not todo:
            self.result.missing.append("(no controls found)")
        in_dialog = self.within is not None
        for control in todo:
            target = locate(tree, self.within, control)
            if target is None:
                self.result.missing.append(f"{control['role']} {control['name']!r}")
                continue
            x, y, w, h = target["extents"]
            self.app.click_at(x + w / 2, y + h / 2)
            self.result.clicked += 1
            if not self.app.running():
                self.result.fail(control, "crash",
                                 f"the app exited with {self.app.process.returncode}")
                tree = self._show(fresh=True)
                continue
            try:
                self.app.wait_idle(timeout=10)
            except (AssertionError, Exception) as error:  # noqa: BLE001
                self.result.fail(control, "hang", f"not idle 10 s after the click: {error}")
            for line in self._new_problems():
                self.result.fail(control, "qml-error", line.strip()[:300])
            if in_dialog and control["setting"]:
                dialog = self.app.state()["active_window"]["dialog"]
                if self.within and self._dialog_expected() and dialog is None:
                    self.result.fail(control, "closed", f"{self.within} closed after the click")
            restored = self._restore()
            if restored is not None:
                self.result.changed.append(f"{control['role']} {control['name']!r}")
                tree = restored
        if self.app is not None:
            self.app.stop()
        self._write()
        return self.result

    def _dialog_expected(self) -> bool:
        return json.loads(self.base).get("dialog") not in (None, "")

    def _write(self) -> None:
        self.report_dir.mkdir(parents=True, exist_ok=True)
        safe = re.sub(r"[^A-Za-z0-9_.-]+", "_", self.name)
        (self.report_dir / f"{safe}.json").write_text(json.dumps(self.result.__dict__, indent=2))


def summary(result: Result) -> str:
    lines = [f"{result.screen}: {result.clicked} clicked, {len(result.failures)} problems, "
             f"{len(result.changed)} changed the screen, {len(result.missing)} not found"]
    lines += [f"  {f['kind']}: {f['control']}: {f['detail']}" for f in result.failures]
    return "\n".join(lines)


__all__ = ["Sweep", "summary", "controls", "WaitTimeout", "wait_for"]
