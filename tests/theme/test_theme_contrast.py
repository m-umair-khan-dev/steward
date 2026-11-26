"""Theme colours meet WCAG 2.2 AA (P11-T03 to T05).

Reads the colours from crates/app/qml/Theme.qml and repeats its OKLCH and
contrast maths (ensureContrast, fromOklch) in Python, then checks every rule
the theme promises: text roles 4.5:1 on every surface, white on every accent
fill and file badge 4.5:1 (Ubuntu's own accents too), kind colours 3:1 on
dark cards, red kept for danger, and no colour written outside Theme.qml.
"""

import math
import re
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
QML = REPO / "crates" / "app" / "qml"
THEME = (QML / "Theme.qml").read_text()

# --- the maths, as in Theme.qml ----------------------------------------------------------------

def rgb(hexcolor):
    h = hexcolor.lstrip("#")
    return [int(h[i:i + 2], 16) / 255 for i in (0, 2, 4)]


def _lin(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def _gam(c):
    return 12.92 * c if c <= 0.0031308 else 1.055 * c ** (1 / 2.4) - 0.055


def luminance(c):
    r, g, b = (_lin(v) for v in c)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def contrast(a, b):
    x, y = luminance(a), luminance(b)
    return (max(x, y) + 0.05) / (min(x, y) + 0.05)


def to_oklch(c):
    r, g, b = (_lin(v) for v in c)
    l = math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b)
    m = math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b)
    s = math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b)
    L = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s
    A = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s
    B = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s
    return L, math.hypot(A, B), math.atan2(B, A)


def _rgb(L, C, h):
    A, B = C * math.cos(h), C * math.sin(h)
    l = (L + 0.3963377774 * A + 0.2158037573 * B) ** 3
    m = (L - 0.1055613458 * A - 0.0638541728 * B) ** 3
    s = (L - 0.0894841775 * A - 1.2914855480 * B) ** 3
    return [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
            -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
            -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s]


def from_oklch(L, C, h):
    out = _rgb(L, C, h)
    while C > 0 and any(v < -0.0001 or v > 1.0001 for v in out):
        C = max(0, C - 0.004)
        out = _rgb(L, C, h)
    return [min(1, max(0, _gam(min(1, max(0, v))))) for v in out]


def ensure_contrast(c, against, ratio):
    if contrast(c, against) >= ratio:
        return c
    lighten = luminance(against) < 0.18
    L, C, h = to_oklch(c)
    out = c
    for _ in range(200):
        if contrast(out, against) >= ratio:
            break
        L = min(1, L + 0.005) if lighten else max(0, L - 0.005)
        out = from_oklch(L, C, h)
        if L <= 0 or L >= 1:
            break
    return out


WHITE = rgb("#ffffff")

# --- what Theme.qml says -------------------------------------------------------------------------

def picked(role):
    """pick(warmLight, warmDark, plainLight, plainDark) for a role."""
    found = re.search(rf'property color {role}: pick\(("#\w+"), ("#\w+"), ("#\w+"), ("#\w+")\)', THEME)
    assert found, f"{role} is not a pick(...) role"
    return dict(zip(("warm light", "warm dark", "plain light", "plain dark"),
                    (rgb(v.strip('"')) for v in found.groups())))


SURFACES = {role: picked(role) for role in ("bg", "pane", "sidebar", "card", "sunken", "paneSide")}
ACCENTS = {m.group(1): [rgb(m.group(i)) for i in range(2, 6)]
           for m in re.finditer(r'(\w+):\s+\["(#\w+)", "(#\w+)", "(#\w+)", "(#\w+)"\]', THEME)}
KINDS = {k: rgb(v) for k, v in re.findall(r'(\w+): "(#\w+)"', THEME.split("kindBase")[1].split("})")[0])}
MODES = ("warm light", "warm dark", "plain light", "plain dark")
# Ubuntu's accent colours (Settings › Appearance) and GNOME's blue.
SYSTEM_ACCENTS = {"orange": "#e95420", "bark": "#787859", "sage": "#657b69", "olive": "#4b8501",
                  "viridian": "#03875b", "prussian": "#308280", "blue": "#0073e5", "purple": "#7764d8",
                  "magenta": "#b34cb3", "red": "#da3450", "yellow": "#c88800", "gnome blue": "#3584e4"}


def worst(color, mode):
    return min(contrast(color, s[mode]) for s in SURFACES.values())


@pytest.mark.parametrize("mode", MODES)
@pytest.mark.parametrize("role,ratio", [("ink", 7.0), ("muted", 4.5), ("faint", 4.5)])
def test_text_roles_are_readable_on_every_surface(role, ratio, mode):
    assert worst(picked(role)[mode], mode) >= ratio


@pytest.mark.parametrize("mode", MODES)
def test_text_levels_stay_distinct(mode):
    L = {r: to_oklch(picked(r)[mode])[0] for r in ("ink", "muted", "faint")}
    assert abs(L["ink"] - L["muted"]) > 0.08 and abs(L["muted"] - L["faint"]) > 0.06


@pytest.mark.parametrize("name", ["steward", "blue", "teal", "green", "orange", "rose", "graphite"])
@pytest.mark.parametrize("dark", [False, True])
def test_white_on_every_accent_fill_and_accent_text(name, dark):
    accent, text = ACCENTS[name][1 if dark else 0], ACCENTS[name][3 if dark else 2]
    mode = ("warm " if name == "steward" else "plain ") + ("dark" if dark else "light")
    fill = ensure_contrast(accent, WHITE, 4.6)
    assert contrast(WHITE, fill) >= 4.5
    assert contrast(fill, SURFACES["bg"][mode]) >= 3.0, "the fill still stands out"
    assert worst(ensure_contrast(text, SURFACES["sunken"][mode], 4.5), mode) >= 4.5


@pytest.mark.parametrize("name", list(SYSTEM_ACCENTS))
@pytest.mark.parametrize("dark", [False, True])
def test_system_accents_get_readable_shades(name, dark):
    base = rgb(SYSTEM_ACCENTS[name])
    mode = "plain dark" if dark else "plain light"
    text = ensure_contrast(base, SURFACES["sunken"][mode], 4.5)
    assert worst(text, mode) >= 4.5
    assert contrast(WHITE, ensure_contrast(base, WHITE, 4.6)) >= 4.5


@pytest.mark.parametrize("kind", list(KINDS))
def test_kind_colours(kind):
    base = KINDS[kind]
    assert contrast(WHITE, ensure_contrast(base, WHITE, 4.5)) >= 4.5, "badge text"
    for mode in ("warm dark", "plain dark"):
        assert contrast(ensure_contrast(base, SURFACES["card"][mode], 3.0), SURFACES["card"][mode]) >= 3.0
    L, C, h = to_oklch(base)
    assert abs(to_oklch(from_oklch(0.78, C * 0.75, h))[0] - 0.78) < 0.01, "light tints share one lightness"


def test_red_is_kept_for_danger():
    danger = rgb(re.search(r'property color danger: "(#\w+)"', THEME).group(1))
    hue = lambda c: math.degrees(to_oklch(c)[2]) % 360
    assert contrast(WHITE, danger) >= 4.5
    for kind, color in KINDS.items():
        far = abs(hue(color) - hue(danger)) > 12 or to_oklch(color)[1] < 0.08
        assert far, f"{kind} looks like the danger red"


def test_colours_are_only_written_in_theme():
    allowed = {"Theme.qml", "FxShadow.qml"}  # FxShadow: the effect's default, always overridden
    pattern = re.compile(r'"#[0-9a-fA-F]{6}"|Qt\.rgba\(|Qt\.hsla\(|"white"|"black"')
    found = [f"{f.name}:{n}" for f in QML.glob("*.qml") if f.name not in allowed
             for n, line in enumerate(f.read_text().splitlines(), 1) if pattern.search(line)]
    assert not found, "use a Theme role: " + ", ".join(found)
