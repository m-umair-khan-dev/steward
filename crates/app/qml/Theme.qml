pragma Singleton
import QtQuick

// Colour, size and motion tokens (DESIGN.md §3.6, ADR 0008).
//
// Colours are roles, named for what they do (ink, muted, danger, onAccent),
// never for how they look; QML elsewhere uses only these (P11-T04). Every
// text role meets WCAG 2.2 AA on every surface, 4.5:1, and every icon and
// control colour 3:1 (P11-T03). Colours that depend on the accent or the
// system's colour are worked out here at run time, in OKLCH, so lighter and
// darker shades keep their hue and look evenly spaced across hues (P11-T05).
QtObject {
    id: theme
    property bool dark: false
    property bool systemAccent: false
    property color systemAccentColor: "#77216f"
    // The app's own colours (Preferences › Appearance › Colour). Each has a
    // shade for light and for dark: [accent light, accent dark, text light, text dark].
    // "steward" is the default and matches the app icon: aubergine on warm
    // cream, with the icon's orange and amber layers.
    property string accentName: "steward"
    readonly property var accents: ({
        steward:  ["#77216f", "#b45aa9", "#661c5f", "#e3a6db"],
        blue:     ["#2f6fdb", "#4d8af0", "#2459b8", "#8ab4f8"],
        teal:     ["#12858f", "#2aa5ad", "#0d6d75", "#6fd0d6"],
        green:    ["#2e8b57", "#45a96f", "#23704a", "#82d1a2"],
        orange:   ["#d9690f", "#ef8a33", "#b4560a", "#f6b37a"],
        rose:     ["#cc3d6e", "#e25b8a", "#ab2d5a", "#f19ab9"],
        graphite: ["#4f5563", "#8a90a0", "#3e4350", "#c3c7d1"]
    })
    readonly property var accentSet: accents[accentName] || accents.steward
    property bool motion: true

    // --- surfaces ---------------------------------------------------------------
    // The default colours take the icon's warm cream; Ubuntu's accent and the
    // other colours sit on plain neutrals. sunken is the darkest light
    // surface (lightest dark one): text roles are measured against it.
    readonly property bool warm: !systemAccent && accentName === "steward"
    function pick(warmLight, warmDark, plainLight, plainDark) {
        return warm ? (dark ? warmDark : warmLight) : (dark ? plainDark : plainLight)
    }
    readonly property color bg: pick("#f6f1ea", "#120d11", "#f4f4f6", "#0e0e10")
    readonly property color pane: pick("#fffdf9", "#1b1519", "#ffffff", "#161618")
    readonly property color sidebar: pick("#f3ede5", "#171215", "#f2f2f4", "#131315")
    readonly property color card: pick("#fffdf9", "#241d22", "#ffffff", "#1f1f22")
    readonly property color sunken: pick("#f0e9e0", "#2a2228", "#efeff2", "#252528")
    readonly property color paneSide: pick("#f9f5ef", "#191417", "#f8f8fa", "#141416")

    // --- text -------------------------------------------------------------------
    // Three levels, all readable: ink 13:1 and more, muted about 7:1, faint
    // about 4.6:1 on sunken. disabled is for controls that cannot be used
    // (WCAG exempts them) and nothing else.
    readonly property color ink: pick("#26191f", "#f3ede8", "#18181c", "#ededf0")
    readonly property color muted: pick("#57494f", "#b8acb1", "#4f4f57", "#afafb7")
    readonly property color faint: pick("#71656d", "#96898f", "#6b6b73", "#8d8d97")
    readonly property color disabled: pick("#958890", "#7f7379", "#8a8a92", "#6f6f78")

    readonly property color line: dark ? Qt.rgba(1, 1, 1, 0.07) : alpha(ink, 0.08)
    readonly property color track: dark ? Qt.rgba(1, 1, 1, 0.06) : alpha(ink, 0.05)
    readonly property color hover: dark ? Qt.rgba(1, 1, 1, 0.05) : alpha(ink, 0.04)
    // The pill behind a segmented control's choice.
    readonly property color segment: pick("#ece6dd", "#2e262c", "#ececf2", "#2c2a3d")

    // --- accent -----------------------------------------------------------------
    // accent marks the selection and focus; accentText is the accent as text;
    // accentFill is the accent behind white (onAccent) text and icons. The
    // system's colour comes as one value, so its shades are worked out here
    // (libadwaita does the same): Ubuntu's orange as text would be 3.6:1.
    readonly property color accent: systemAccent ? systemAccentColor : accentSet[dark ? 1 : 0]
    readonly property color accentText: ensureContrast(systemAccent ? systemAccentColor : accentSet[dark ? 3 : 2],
                                                       sunken, 4.5)
    readonly property color accentFill: ensureContrast(accent, onAccent, 4.6)
    readonly property color accentFillHover: shiftLightness(accentFill, -0.05)
    readonly property color onAccent: "#ffffff"

    // --- meaning ----------------------------------------------------------------
    // danger is only for destroying things (Empty Trash, Delete) and errors:
    // no file kind or decoration uses red, so red keeps its warning.
    readonly property color danger: "#c01c28"
    readonly property color dangerHover: "#a7011a"
    readonly property color dangerText: dark ? "#f25352" : "#c01c28"
    readonly property color success: dark ? "#409c68" : "#147948"
    readonly property color successFill: alpha("#2c8a57", dark ? 0.22 : 0.12)
    readonly property color successLine: alpha("#2c8a57", 0.35)
    readonly property color successText: dark ? "#8fd6ad" : "#1f6b42"
    readonly property color warning: dark ? "#c07c1b" : "#965d04"
    // Usage charts (AI Agents): one series colour.
    readonly property color chart: "#d97757"

    // --- overlays and effects ---------------------------------------------------
    // The dimmed backdrop behind dialogs, search and Peek, at a strength.
    function scrim(strength) { return Qt.rgba(0.05, 0.04, 0.1, strength) }
    // White on a coloured fill (folder glass, chips over a picture), at a strength.
    function onFill(strength) { return Qt.rgba(1, 1, 1, strength) }
    readonly property color caption: Qt.rgba(0.1, 0.1, 0.15, 0.7)   // text box over a picture
    readonly property color shadow: "#1a1240"
    readonly property color toastBg: pick("#2e2128", "#3a2e36", "#26262b", "#323238")
    readonly property color paper: "#efedf8"                         // a sheet peeking out of a folder
    // The drawn computer on About This Computer: [body, screen top, screen bottom, stand].
    readonly property var device: dark ? ["#2b2840", "#55536a", "#3a384c", "#55536a"]
                                       : ["#2b2840", "#d9d8e2", "#b9b7c6", "#c9c7d4"]

    // A link without a picture: a gradient from its hue (degrees), dark
    // enough that the white initial on it reads as large text (3:1).
    function placeholder(hue, step) {
        const L = (dark ? 0.42 : 0.62) - step * 0.06
        return fromOklch(L, 0.13, (hue % 360) * Math.PI / 180)
    }

    // Accepts a colour or a colour name ("#6bbe91"): names have no r, g, b.
    function alpha(c, a) { const k = Qt.lighter(c, 1.0); return Qt.rgba(k.r, k.g, k.b, a) }

    // --- file kinds -------------------------------------------------------------
    // [deep, light]. Folders take the chosen colour; the other kinds keep
    // theirs. light is the same hue at one OKLCH lightness for every kind, so
    // the light tints look equally light. In dark mode a deep colour that
    // would sink into the cards (under 3:1) is lifted until it shows.
    // PDFs are terracotta, not red: red is kept for danger.
    readonly property color folderDeep: systemAccent ? systemAccentColor : accentSet[0]
    readonly property var kindBase: ({
        folders: folderDeep, images: "#dc7409", videos: "#13778a", audio: "#be3d77",
        pdfs: "#b4532a", documents: "#2c6cd6", archives: "#2c8a57", installers: "#4c5361",
        code: "#3a49b5", other: "#7a7d86"
    })
    readonly property var tints: {
        const out = {}
        for (const kind in kindBase) {
            const base = Qt.lighter(kindBase[kind], 1.0)
            const deep = dark ? ensureContrast(base, card, 3.0) : base
            const lch = toOklch(base)
            // [deep, light, badge]: badge carries white text (4.5:1).
            out[kind] = [deep, fromOklch(0.78, lch[1] * 0.75, lch[2]), ensureContrast(base, onAccent, 4.5)]
        }
        return out
    }
    function deep(kind) { return (tints[kind] || tints.other)[0] }
    function light(kind) { return (tints[kind] || tints.other)[1] }
    // A kind's colour behind white text or a white glyph (file badges, chips).
    function badge(kind) { return (tints[kind] || tints.other)[2] }

    // --- colour maths (WCAG 2.2 contrast, OKLCH) --------------------------------
    function _lin(c) { return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4) }
    function _gam(c) { return c <= 0.0031308 ? 12.92 * c : 1.055 * Math.pow(c, 1 / 2.4) - 0.055 }
    function luminance(c) {
        const k = Qt.lighter(c, 1.0)
        return 0.2126 * _lin(k.r) + 0.7152 * _lin(k.g) + 0.0722 * _lin(k.b)
    }
    function contrast(a, b) {
        const x = luminance(a), y = luminance(b)
        return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05)
    }
    // [L, C, h] (Björn Ottosson's OKLab, in polar form).
    function toOklch(c) {
        const k = Qt.lighter(c, 1.0)
        const r = _lin(k.r), g = _lin(k.g), b = _lin(k.b)
        const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b)
        const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b)
        const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b)
        const L = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s
        const A = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s
        const B = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s
        return [L, Math.sqrt(A * A + B * B), Math.atan2(B, A)]
    }
    function _rgb(L, C, h) {
        const A = C * Math.cos(h), B = C * Math.sin(h)
        const l = Math.pow(L + 0.3963377774 * A + 0.2158037573 * B, 3)
        const m = Math.pow(L - 0.1055613458 * A - 0.0638541728 * B, 3)
        const s = Math.pow(L - 0.0894841775 * A - 1.2914855480 * B, 3)
        return [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
                -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
                -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s]
    }
    // A colour from OKLCH; out of the screen's range, chroma is lowered
    // (the hue and lightness stay) until it fits.
    function fromOklch(L, C, h) {
        let rgb = _rgb(L, C, h)
        while (C > 0 && rgb.some(v => v < -0.0001 || v > 1.0001)) {
            C = Math.max(0, C - 0.004)
            rgb = _rgb(L, C, h)
        }
        const clamp = v => Math.min(1, Math.max(0, _gam(Math.min(1, Math.max(0, v)))))
        return Qt.rgba(clamp(rgb[0]), clamp(rgb[1]), clamp(rgb[2]), 1)
    }
    // The same colour, lighter (+) or darker (-) in OKLCH lightness.
    function shiftLightness(c, delta) {
        const lch = toOklch(c)
        return fromOklch(Math.min(1, Math.max(0, lch[0] + delta)), lch[1], lch[2])
    }
    // c, moved in lightness only (away from `against`) until it has at
    // least `ratio` contrast with it. Unchanged if it already has.
    function ensureContrast(c, against, ratio) {
        if (contrast(c, against) >= ratio) return c
        const lighten = luminance(against) < 0.18
        const lch = toOklch(c)
        let L = lch[0], out = c
        for (let i = 0; i < 200 && contrast(out, against) < ratio; i++) {
            L = lighten ? Math.min(1, L + 0.005) : Math.max(0, L - 0.005)
            out = fromOklch(L, lch[1], lch[2])
            if (L <= 0 || L >= 1) break
        }
        return out
    }

    // QtQuick.Effects (blur, shadows, GPU tinting) is in Qt 6.5+. Ubuntu
    // 24.04 has Qt 6.4: Main.qml checks at start-up and sets this; without it
    // the Fx* components and Icon.qml use their fallbacks.
    property bool effects: false

    readonly property int radius: 12
    readonly property int springMs: 220
}
