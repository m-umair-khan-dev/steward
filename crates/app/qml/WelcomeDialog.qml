import QtQuick
import QtQuick.Controls.Basic

// The first-run welcome (P6-T01): a messy folder sorted before your eyes,
// a short tour of the keys that matter, and "make it yours". Shown once.
// The demo is a drawing: it touches no files.
Sheet {
    id: root
    property var info: ({ step: "demo", index: 0, count: 3, sorted: false, tour: [] })
    property var prefs: ({})
    title: p.t("Welcome to Cairn")
    cardWidth: 620

    // Loose files of the demo, and the kind folder each one belongs in.
    readonly property var files: [
        { name: "beach.jpg", kind: "images", x: 0.04, y: 0.10, r: -8 },
        { name: "invoice.pdf", kind: "pdfs", x: 0.30, y: 0.04, r: 6 },
        { name: "notes.odt", kind: "documents", x: 0.58, y: 0.14, r: -4 },
        { name: "trip.mp4", kind: "videos", x: 0.80, y: 0.06, r: 9 },
        { name: "logo.png", kind: "images", x: 0.12, y: 0.52, r: 7 },
        { name: "lease.pdf", kind: "pdfs", x: 0.42, y: 0.46, r: -10 },
        { name: "budget.ods", kind: "documents", x: 0.66, y: 0.56, r: 5 },
        { name: "talk.webm", kind: "videos", x: 0.86, y: 0.48, r: -6 }
    ]
    readonly property var kinds: [
        { id: "images", label: p.t("Images") }, { id: "pdfs", label: p.t("PDFs") },
        { id: "documents", label: p.t("Documents") }, { id: "videos", label: p.t("Videos") }
    ]

    Column {
        width: parent.width
        spacing: 14

        // Step dots.
        Row {
            spacing: 6
            Accessible.role: Accessible.ProgressBar
            Accessible.name: p.t("Step ") + (root.info.index + 1) + p.t(" of ") + root.info.count
            Repeater {
                model: root.info.count || 3
                delegate: Rectangle {
                    required property int index
                    width: index === root.info.index ? 22 : 8; height: 8; radius: 4
                    color: index === root.info.index ? Theme.accent : Theme.track
                    Behavior on width { enabled: Theme.motion; NumberAnimation { duration: 160 } }
                }
            }
        }

        // --- 1. the demo -----------------------------------------------------------
        Column {
            visible: root.info.step === "demo"
            width: parent.width
            spacing: 10
            Text { text: p.t("A messy folder, sorted"); color: Theme.ink; font.pixelSize: 22; font.weight: Font.Bold
                   Accessible.role: Accessible.Heading; Accessible.name: text }
            Text {
                width: parent.width; wrapMode: Text.WordWrap; color: Theme.muted; font.pixelSize: 14
                text: root.info.sorted
                      ? "That is Tidy. In your own folders it shows the plan first, and one Undo puts everything back."
                      : "Downloads fill up fast. Cairn sorts loose files into folders by kind, and moves nothing until you agree."
            }
            Rectangle {
                id: stage
                width: parent.width; height: 210
                radius: 14; color: Theme.sunken
                clip: true
                Accessible.role: Accessible.Graphic
                Accessible.name: root.info.sorted ? p.t("Sorted folder") : p.t("Messy folder")
                // Kind folders appear when sorted.
                Row {
                    anchors.centerIn: parent
                    spacing: 34
                    opacity: root.info.sorted ? 1 : 0
                    scale: root.info.sorted ? 1 : 0.7
                    Behavior on opacity { NumberAnimation { duration: Theme.motion ? 320 : 0 } }
                    Behavior on scale { enabled: Theme.motion; SpringAnimation { spring: 3; damping: 0.3 } }
                    Repeater {
                        model: root.kinds
                        delegate: Column {
                            required property var modelData
                            spacing: 6
                            MiniFolder { size: 72; kind: parent.modelData.id }
                            Text { anchors.horizontalCenter: parent.horizontalCenter; text: parent.modelData.label + " · 2"
                                   color: Theme.ink; font.pixelSize: 12; font.weight: Font.DemiBold }
                        }
                    }
                }
                // Loose files fly into their folders.
                Repeater {
                    model: root.files
                    delegate: Rectangle {
                        id: file
                        required property var modelData
                        required property int index
                        readonly property int target: root.kinds.findIndex(k => k.id === modelData.kind)
                        width: 104; height: 34; radius: 8
                        color: Theme.card; border.color: Theme.line
                        x: root.info.sorted ? stage.width / 2 - 2 * 106 + target * 106 + 4 : modelData.x * (stage.width - width)
                        y: root.info.sorted ? stage.height / 2 - 30 : 14 + modelData.y * (stage.height - 60)
                        rotation: root.info.sorted ? 0 : modelData.r
                        scale: root.info.sorted ? 0.2 : 1
                        opacity: root.info.sorted ? 0 : 1
                        Behavior on x { NumberAnimation { duration: Theme.motion ? 520 + file.index * 40 : 0; easing.type: Easing.InOutCubic } }
                        Behavior on y { NumberAnimation { duration: Theme.motion ? 520 + file.index * 40 : 0; easing.type: Easing.InOutCubic } }
                        Behavior on scale { NumberAnimation { duration: Theme.motion ? 560 : 0 } }
                        Behavior on opacity { NumberAnimation { duration: Theme.motion ? 600 : 0 } }
                        Behavior on rotation { NumberAnimation { duration: Theme.motion ? 400 : 0 } }
                        Rectangle { x: 8; anchors.verticalCenter: parent.verticalCenter; width: 14; height: 18; radius: 3
                                    color: Theme.deep(file.modelData.kind) }
                        Text { x: 28; anchors.verticalCenter: parent.verticalCenter; text: file.modelData.name
                               color: Theme.ink; font.pixelSize: 11 }
                    }
                }
            }
            PillButton {
                text: root.info.sorted ? p.t("Mess It Up Again") : p.t("Sort It")
                onClicked: p.activate("win.welcome-step", root.info.sorted ? "unsort" : "sort")
            }
        }

        // --- 2. the tour -----------------------------------------------------------
        Column {
            visible: root.info.step === "tour"
            width: parent.width
            spacing: 10
            Text { text: p.t("Four keys worth knowing"); color: Theme.ink; font.pixelSize: 22; font.weight: Font.Bold
                   Accessible.role: Accessible.Heading; Accessible.name: text }
            Repeater {
                model: root.info.tour || []
                delegate: Rectangle {
                    required property var modelData
                    width: parent.width; height: 62; radius: 12
                    color: Theme.sunken
                    Accessible.role: Accessible.StaticText
                    Accessible.name: modelData.keys + ": " + modelData.title
                    Rectangle {
                        x: 14; anchors.verticalCenter: parent.verticalCenter
                        width: 130; height: 32; radius: 8; color: Theme.card; border.color: Theme.line
                        Text { anchors.centerIn: parent; text: parent.parent.modelData.keys; color: Theme.ink
                               font.pixelSize: 13; font.weight: Font.DemiBold; font.family: "monospace" }
                    }
                    Column {
                        x: 160; anchors.verticalCenter: parent.verticalCenter
                        width: parent.width - 176
                        Text { text: parent.parent.modelData.title; color: Theme.ink; font.pixelSize: 15; font.weight: Font.DemiBold }
                        Text { width: parent.width; text: parent.parent.modelData.text; color: Theme.muted; font.pixelSize: 13
                               elide: Text.ElideRight }
                    }
                }
            }
        }

        // --- 3. make it yours ------------------------------------------------------
        Column {
            visible: root.info.step === "yours"
            width: parent.width
            spacing: 12
            Text { text: p.t("Make it yours"); color: Theme.ink; font.pixelSize: 22; font.weight: Font.Bold
                   Accessible.role: Accessible.Heading; Accessible.name: text }
            Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.muted; font.pixelSize: 14
                   text: p.t("You can change these any time in Preferences (Ctrl+,).") }
            Item {
                width: parent.width; height: 44
                Text { anchors.verticalCenter: parent.verticalCenter; text: p.t("Style"); color: Theme.ink; font.pixelSize: 14 }
                Segmented {
                    anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                    name: p.t("Style")
                    options: [{ id: "system", label: p.t("System") }, { id: "light", label: p.t("Light") }, { id: "dark", label: p.t("Dark") }]
                    current: root.prefs.colorScheme || "system"
                    onPicked: id => p.setpref("color-scheme", id)
                }
            }
            Item {
                width: parent.width; height: 44
                Text { anchors.verticalCenter: parent.verticalCenter; text: p.t("Accent"); color: Theme.ink; font.pixelSize: 14 }
                Segmented {
                    anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                    name: p.t("Accent")
                    options: [{ id: "cairn", label: p.t("Cairn") }, { id: "system", label: p.t("System") }]
                    current: root.prefs.accent || "cairn"
                    onPicked: id => p.setpref("accent", id)
                }
            }
            Item {
                width: parent.width; height: 44
                Text { anchors.verticalCenter: parent.verticalCenter; text: p.t("Card size"); color: Theme.ink; font.pixelSize: 14 }
                Segmented {
                    anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                    name: p.t("Card size")
                    options: [{ id: "small", label: p.t("Small") }, { id: "medium", label: p.t("Medium") }, { id: "large", label: p.t("Large") }]
                    current: root.prefs.rowSize || "medium"
                    onPicked: id => p.setpref("row-size", id)
                }
            }
        }

        // Buttons.
        Item {
            width: parent.width; height: 40
            PillButton {
                visible: root.info.index < root.info.count - 1
                text: p.t("Skip")
                onClicked: p.activate("win.welcome-done", "")
            }
            Row {
                anchors.right: parent.right
                spacing: 8
                PillButton { visible: root.info.index > 0; text: p.t("Back"); onClicked: p.activate("win.welcome-step", "back") }
                PillButton {
                    primary: true
                    text: root.info.index < root.info.count - 1 ? p.t("Next") : p.t("Start Using Cairn")
                    onClicked: p.activate(root.info.index < root.info.count - 1 ? "win.welcome-step" : "win.welcome-done",
                                          root.info.index < root.info.count - 1 ? "next" : "")
                }
            }
        }
    }
}
