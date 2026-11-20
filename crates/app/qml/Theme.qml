pragma Singleton
import QtQuick

// Colour, size and motion tokens (DESIGN.md §3.6, ADR 0008).
QtObject {
    property bool dark: false
    property bool systemAccent: false
    property color systemAccentColor: "#6b56dc"
    property bool motion: true

    readonly property color bg: dark ? "#0c0b14" : "#f4f3f9"
    readonly property color pane: dark ? "#15141f" : "#ffffff"
    readonly property color sidebar: dark ? "#12111b" : "#f3f2f9"
    readonly property color card: dark ? "#1e1d2b" : "#ffffff"
    readonly property color sunken: dark ? "#232232" : "#f1f0f6"
    readonly property color paneSide: dark ? "#13121c" : "#f8f7fb"
    readonly property color ink: dark ? "#eeecf7" : "#16132a"
    readonly property color muted: dark ? "#a6a2bc" : "#5d5a72"
    readonly property color faint: dark ? "#6f6b85" : "#8a879a"
    readonly property color line: dark ? Qt.rgba(1, 1, 1, 0.07) : Qt.rgba(0.086, 0.075, 0.165, 0.08)
    readonly property color track: dark ? Qt.rgba(1, 1, 1, 0.06) : Qt.rgba(0.086, 0.075, 0.165, 0.05)
    readonly property color hover: dark ? Qt.rgba(1, 1, 1, 0.05) : Qt.rgba(0.086, 0.075, 0.165, 0.04)
    readonly property color accent: systemAccent ? systemAccentColor : (dark ? "#7b66e6" : "#6b56dc")
    readonly property color accentText: systemAccent ? systemAccentColor : (dark ? "#a897f3" : "#5a45cc")
    readonly property color toastBg: dark ? "#2e2b45" : "#26233a"

    // Accepts a colour or a colour name ("#6bbe91"): names have no r, g, b.
    function alpha(c, a) { const k = Qt.lighter(c, 1.0); return Qt.rgba(k.r, k.g, k.b, a) }

    // Kind tints: [deep, light].
    readonly property var tints: ({
        folders: ["#6a55db", "#a497f2"], images: ["#dc7409", "#f4ae55"],
        videos: ["#13778a", "#55afbf"], audio: ["#be3d77", "#e584af"],
        pdfs: ["#c53434", "#e77474"], documents: ["#2c6cd6", "#73a2ee"],
        archives: ["#2c8a57", "#6bbe91"], installers: ["#4c5361", "#8a919d"],
        code: ["#3a49b5", "#7d88df"], other: ["#7a7d86", "#b1b3b9"]
    })
    function deep(kind) { return (tints[kind] || tints.other)[0] }
    function light(kind) { return (tints[kind] || tints.other)[1] }

    readonly property int radius: 12
    readonly property int springMs: 220
}
