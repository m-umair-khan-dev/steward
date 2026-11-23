pragma Singleton
import QtQuick

// Colour, size and motion tokens (DESIGN.md §3.6, ADR 0008).
QtObject {
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

    // Surfaces. The default colours take the icon's warm cream; Ubuntu's
    // accent and the other colours sit on plain neutrals.
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
    readonly property color ink: pick("#26191f", "#f3ede8", "#18181c", "#ededf0")
    readonly property color muted: pick("#6b5d63", "#b5a9ae", "#5e5e66", "#a5a5ad")
    readonly property color faint: pick("#958890", "#7f7379", "#8a8a92", "#6f6f78")
    readonly property color line: dark ? Qt.rgba(1, 1, 1, 0.07) : alpha(ink, 0.08)
    readonly property color track: dark ? Qt.rgba(1, 1, 1, 0.06) : alpha(ink, 0.05)
    readonly property color hover: dark ? Qt.rgba(1, 1, 1, 0.05) : alpha(ink, 0.04)
    readonly property color accent: systemAccent ? systemAccentColor : accentSet[dark ? 1 : 0]
    readonly property color accentText: systemAccent ? systemAccentColor : accentSet[dark ? 3 : 2]
    readonly property color toastBg: pick("#2e2128", "#3a2e36", "#26262b", "#323238")

    // Accepts a colour or a colour name ("#6bbe91"): names have no r, g, b.
    function alpha(c, a) { const k = Qt.lighter(c, 1.0); return Qt.rgba(k.r, k.g, k.b, a) }

    // Kind tints: [deep, light].
    // Folders take the chosen colour; the other kinds keep theirs.
    readonly property color folderDeep: systemAccent ? systemAccentColor : accentSet[0]
    readonly property var tints: ({
        folders: [folderDeep, Qt.lighter(folderDeep, 1.45)], images: ["#dc7409", "#f4ae55"],
        videos: ["#13778a", "#55afbf"], audio: ["#be3d77", "#e584af"],
        pdfs: ["#c53434", "#e77474"], documents: ["#2c6cd6", "#73a2ee"],
        archives: ["#2c8a57", "#6bbe91"], installers: ["#4c5361", "#8a919d"],
        code: ["#3a49b5", "#7d88df"], other: ["#7a7d86", "#b1b3b9"]
    })
    function deep(kind) { return (tints[kind] || tints.other)[0] }
    function light(kind) { return (tints[kind] || tints.other)[1] }

    // QtQuick.Effects (blur, shadows, GPU tinting) is in Qt 6.5+. Ubuntu
    // 24.04 has Qt 6.4: Main.qml checks at start-up and sets this; without it
    // the Fx* components and Icon.qml use their fallbacks.
    property bool effects: false

    readonly property int radius: 12
    readonly property int springMs: 220
}
