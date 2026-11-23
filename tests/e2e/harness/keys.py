"""Real key presses, injected through mutter's remote-desktop D-Bus interface
(ADR 0003). Needs the mutter compositor; weston has no such interface."""

from gi.repository import Gio, GLib

DEST = "org.gnome.Mutter.RemoteDesktop"
ROOT = "/org/gnome/Mutter/RemoteDesktop"
SESSION_IFACE = "org.gnome.Mutter.RemoteDesktop.Session"

# Linux evdev key codes (linux/input-event-codes.h), US layout.
_LETTERS = "qwertyuiop", "asdfghjkl", "zxcvbnm"
_ROW_START = (16, 30, 44)
KEYCODES: dict[str, int] = {
    letter: start + index
    for row, start in zip(_LETTERS, _ROW_START)
    for index, letter in enumerate(row)
}
KEYCODES.update({str(n): 1 + n for n in range(1, 10)})
KEYCODES.update({f"f{n}": 58 + n for n in range(1, 11)})
KEYCODES.update({
    "0": 11, "minus": 12, "plus": 13, "equal": 13, "backspace": 14, "tab": 15, "return": 28,
    "enter": 28, "escape": 1, "esc": 1, "space": 57, "comma": 51, "period": 52, "slash": 53,
    "f10": 68, "f11": 87, "menu": 127, "f12": 88, "home": 102, "up": 103, "pageup": 104, "left": 105, "right": 106,
    "end": 107, "down": 108, "pagedown": 109, "insert": 110, "delete": 111,
    "ctrl": 29, "control": 29, "shift": 42, "alt": 56, "super": 125, "grave": 41,
    "semicolon": 39, "apostrophe": 40, "backslash": 43, "leftbracket": 26, "rightbracket": 27,
})

# Characters for type_text(): the combination that types each one (US layout).
_CHARS = {" ": "space", "/": "slash", ".": "period", ",": "comma", "-": "minus", "=": "equal",
          "~": "shift+grave", "_": "shift+minus", "+": "shift+equal", ":": "shift+semicolon",
          "\n": "return", "#": "shift+3"}


def combo_for(char: str) -> str:
    if char in _CHARS:
        return _CHARS[char]
    if char.isascii() and char.isalpha():
        return f"shift+{char.lower()}" if char.isupper() else char
    if char.isdigit():
        return char
    raise ValueError(f"cannot type {char!r} with the US key table")
SHIFT = KEYCODES["shift"]


class Keyboard:
    def __init__(self, bus: Gio.DBusConnection) -> None:
        self.bus = bus
        self.session_path: str | None = None

    def _call(self, path: str, iface: str, method: str, args: GLib.Variant | None,
              reply: str | None = None):
        result = self.bus.call_sync(DEST, path, iface, method, args,
                                    GLib.VariantType(reply) if reply else None,
                                    Gio.DBusCallFlags.NONE, 5000, None)
        return result.unpack() if result is not None else None

    def _key(self, code: int, pressed: bool) -> None:
        self._call(self.session_path, SESSION_IFACE, "NotifyKeyboardKeycode",
                   GLib.Variant("(ub)", (code, pressed)))

    def start(self) -> None:
        """Opens a remote-desktop session. Mutter focuses the window on the
        first input event, which is then lost, so a harmless Shift tap goes first."""
        if self.session_path:
            return
        try:
            (self.session_path,) = self._call(ROOT, "org.gnome.Mutter.RemoteDesktop",
                                              "CreateSession", None, "(o)")
        except GLib.Error as error:
            raise AssertionError("real key presses need the mutter compositor "
                                 f"(STEWARD_COMPOSITOR=mutter): {error.message}") from error
        self._call(self.session_path, SESSION_IFACE, "Start", None)
        self._key(SHIFT, True)
        self._key(SHIFT, False)

    def press(self, combo: str) -> None:
        """Presses a combination such as "ctrl+q", "shift+f10" or "escape"."""
        names = [part.strip().lower() for part in combo.split("+") if part.strip()]
        try:
            codes = [KEYCODES[name] for name in names]
        except KeyError as error:
            raise ValueError(f"unknown key {error.args[0]!r} in {combo!r}") from None
        for code in codes:
            self._key(code, True)
        for code in reversed(codes):
            self._key(code, False)
