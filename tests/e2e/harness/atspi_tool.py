"""Reads and acts on an app's accessibility tree over AT-SPI.

Run as a separate process with the test session's environment, because the
AT-SPI library binds to one session bus per process.

    atspi_tool.py PID dump                       JSON tree of the app's accessibles
    atspi_tool.py PID click NAME [ROLE] [WITHIN] perform the default action on a node
    atspi_tool.py PID focus NAME [ROLE] [WITHIN] give a node keyboard focus

ROLE is an AT-SPI role name such as "button" (empty string for any); see ROLE_ALIASES.
WITHIN limits the search to the subtree of the first showing node with that
name, for when the same name appears more than once (e.g. two "Close" buttons).
"""

import json
import sys
import time

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi  # noqa: E402

PREFERRED_ACTIONS = ("click", "activate", "press", "toggle")
TIMEOUT = 10.0

# AT-SPI renamed some roles between releases (Ubuntu 24.04 reports "push
# button" and "panel"; 26.10 reports "button" and "grouping"). Tests use the
# short names; both spellings match.
ROLE_ALIASES = {"push button": "button", "grouping": "panel"}


def role_of(node) -> str:
    role = node.get_role_name()
    return ROLE_ALIASES.get(role, role)


def find_app(pid: int):
    deadline = time.monotonic() + TIMEOUT
    while True:
        desktop = Atspi.get_desktop(0)
        for index in range(desktop.get_child_count()):
            app = desktop.get_child_at_index(index)
            try:
                if app is not None and app.get_process_id() == pid:
                    return app
            except Exception:  # noqa: BLE001 - app may be going away
                continue
        if time.monotonic() > deadline:
            sys.exit(f"no accessible application with pid {pid}")
        time.sleep(0.1)


def actions_of(node) -> list[str]:
    try:
        action = node.get_action_iface()
        if action is None:
            return []
        return [action.get_action_name(i) for i in range(action.get_n_actions())]
    except Exception:  # noqa: BLE001
        return []


def describe(node, depth: int = 0) -> dict:
    states = node.get_state_set()
    children = []
    if depth < 60:
        for index in range(node.get_child_count()):
            child = node.get_child_at_index(index)
            if child is not None:
                children.append(describe(child, depth + 1))
    return {
        "role": role_of(node),
        "name": node.get_name() or "",
        "showing": states.contains(Atspi.StateType.SHOWING),
        "enabled": states.contains(Atspi.StateType.ENABLED),
        "active": states.contains(Atspi.StateType.ACTIVE),
        "actions": actions_of(node),
        "children": children,
    }


def find(node, name: str, role: str | None, need_action: bool = False):
    if (node.get_name() == name and (role is None or role_of(node) == ROLE_ALIASES.get(role, role))
            and node.get_state_set().contains(Atspi.StateType.SHOWING)
            and (not need_action or actions_of(node))):
        return node
    for index in range(node.get_child_count()):
        child = node.get_child_at_index(index)
        if child is not None:
            found = find(child, name, role, need_action)
            if found is not None:
                return found
    return None


def click(app, name: str, role: str | None, within: str | None) -> None:
    deadline = time.monotonic() + TIMEOUT
    while True:
        scope = find(app, within, None) if within else app
        node = find(scope, name, role, need_action=True) if scope is not None else None
        if node is not None:
            names = actions_of(node)
            if names:
                # Qt 6.4 names them "Press" and "Toggle"; newer Qt "press", "toggle".
                lower = [n.lower() for n in names]
                index = next((lower.index(a) for a in PREFERRED_ACTIONS if a in lower), 0)
                node.get_action_iface().do_action(index)
                return
        if time.monotonic() > deadline:
            sys.exit(f"no showing node named {name!r}" + (f" with role {role!r}" if role else "")
                     + (f" inside {within!r}" if within else "") + " that has an action")
        time.sleep(0.1)


def focus(app, name: str, role: str | None, within: str | None) -> None:
    deadline = time.monotonic() + TIMEOUT
    while True:
        scope = find(app, within, None) if within else app
        node = find(scope, name, role) if scope is not None else None
        if node is not None:
            component = node.get_component_iface()
            if component is not None and component.grab_focus():
                return
        if time.monotonic() > deadline:
            sys.exit(f"could not focus {name!r}" + (f" inside {within!r}" if within else ""))
        time.sleep(0.1)


def main() -> None:
    pid, command, *rest = sys.argv[1:]
    app = find_app(int(pid))
    if command == "dump":
        json.dump(describe(app), sys.stdout)
    elif command == "click":
        role = rest[1] if len(rest) > 1 and rest[1] else None
        within = rest[2] if len(rest) > 2 and rest[2] else None
        click(app, rest[0], role, within)
    elif command == "focus":
        role = rest[1] if len(rest) > 1 and rest[1] else None
        within = rest[2] if len(rest) > 2 and rest[2] else None
        focus(app, rest[0], role, within)
    else:
        sys.exit(f"unknown command {command}")


if __name__ == "__main__":
    main()
