import QtQuick
import QtQuick.Effects
import QtQuick.Window
import dev.steward.proof

// The proof window: a Downloads-like folder from steward-core, glass folders,
// files flying into kind folders, a list that collapses and reflows, a toast.
// With STEWARD_CAPTURE_DIR set, a scripted demo runs and saves frames.
Window {
    id: win
    width: 1100
    height: 720
    visible: true
    title: "Steward Qt proof"
    color: "#f4f3f9"

    property bool byKind: false
    property var entries: []          // [{name, kind, size}]
    readonly property var folders: entries.filter(e => e.kind === "folders")
    readonly property var files: entries.filter(e => e.kind !== "folders")
    property var kindGroups: []       // [{kind, count, bytes}]
    readonly property var kindOrder: ["images", "videos", "audio", "pdfs", "documents", "archives", "installers", "code", "other"]
    readonly property var deep: ({ folders: "#6a55db", images: "#dc7409", videos: "#13778a", audio: "#be3d77",
                                   pdfs: "#c53434", documents: "#2c6cd6", archives: "#2c8a57",
                                   installers: "#4c5361", code: "#3a49b5", other: "#7a7d86" })
    readonly property var light: ({ folders: "#a497f2", images: "#f4ae55", videos: "#55afbf", audio: "#e584af",
                                    pdfs: "#e77474", documents: "#73a2ee", archives: "#6bbe91",
                                    installers: "#8a919d", code: "#7d88df", other: "#b1b3b9" })
    readonly property var labels: ({ images: "Images", videos: "Videos", audio: "Audio", pdfs: "PDFs",
                                     documents: "Documents", archives: "Archives", installers: "Installers",
                                     code: "Code", other: "Other" })

    function human(bytes) {
        const units = ["bytes", "kB", "MB", "GB", "TB"]
        let n = Number(bytes), i = 0
        while (n >= 1000 && i < units.length - 1) { n /= 1000; i++ }
        return i === 0 ? n + " bytes" : n.toFixed(1) + " " + units[i]
    }

    function rebuild() {
        const names = lister.names.length ? lister.names.split("\n") : []
        const kinds = lister.kinds.split("\n"), sizes = lister.sizes.split("\n")
        entries = names.map((n, i) => ({ name: n, kind: kinds[i], size: Number(sizes[i]) }))
        const groups = {}
        for (const f of files) {
            groups[f.kind] = groups[f.kind] || { kind: f.kind, count: 0, bytes: 0 }
            groups[f.kind].count++; groups[f.kind].bytes += f.size
        }
        kindGroups = kindOrder.filter(k => groups[k]).map(k => groups[k])
    }

    FolderLister {
        id: lister
        onNamesChanged: win.rebuild()
        Component.onCompleted: load(folder)
    }

    Shortcut { sequence: "Ctrl+K"; onActivated: win.byKind = !win.byKind }
    Shortcut { sequence: "Ctrl+Q"; onActivated: Qt.quit() }
    Shortcut { sequence: "Delete"; onActivated: clean.removeFirst() }
    Shortcut { sequence: "Ctrl+Z"; onActivated: clean.undo() }

    // --- sidebar: an inset rounded panel ----------------------------------------
    Rectangle {
        id: sidebar
        x: 8; y: 8; width: 210; height: parent.height - 16
        radius: 16
        color: "#ecebf4"
        border.color: Qt.rgba(0.09, 0.07, 0.16, 0.08)
        Column {
            x: 16; y: 16; spacing: 6
            Row {
                spacing: 10
                Rectangle {
                    width: 28; height: 28; radius: 9
                    gradient: Gradient { GradientStop { position: 0; color: "#a497f2" } GradientStop { position: 1; color: "#6a55db" } }
                }
                Text { text: "Steward"; font.pixelSize: 19; font.weight: Font.Bold; color: "#16132a"; anchors.verticalCenter: parent.verticalCenter }
            }
            Item { width: 1; height: 10 }
            Text { text: "Favorites"; color: "#8a879a"; font.pixelSize: 12; font.weight: Font.DemiBold }
            Repeater {
                model: ["Home", "Desktop", "Documents", "Downloads", "Pictures"]
                delegate: Rectangle {
                    required property string modelData
                    width: 178; height: 32; radius: 10
                    color: modelData === "Downloads" ? Qt.rgba(0.42, 0.34, 0.86, 0.16) : "transparent"
                    Text { x: 12; anchors.verticalCenter: parent.verticalCenter; text: parent.modelData; color: "#16132a"
                           font.pixelSize: 14; font.weight: parent.modelData === "Downloads" ? Font.Bold : Font.Normal }
                }
            }
        }
    }

    // --- content pane ------------------------------------------------------------
    Rectangle {
        id: content
        x: sidebar.width + 16; y: 0
        width: parent.width - x - 316; height: parent.height
        color: "white"

        Text {
            id: title
            x: 24; y: 22
            text: "Downloads"
            font.pixelSize: 22; font.weight: Font.Bold; color: "#16132a"
        }
        Segmented {
            x: content.width - width - 24; y: 18
            options: ["None", "Kind"]
            current: win.byKind ? 1 : 0
            onPicked: index => win.byKind = index === 1
        }
        Text {
            x: 24; y: 58
            text: lister.status === "ready" ? (win.byKind ? "Grouped by kind" : "Not grouped") : "Loading " + lister.folder
            color: "#8a879a"; font.pixelSize: 12
        }

        Item {
            id: stage
            x: 24; y: 90
            width: content.width - 48; height: content.height - 110
            readonly property int cardW: 196
            readonly property int cardH: 156
            readonly property int gap: 14
            readonly property int columns: Math.max(1, Math.floor((width + gap) / (cardW + gap)))
            readonly property int kindColumns: Math.max(1, Math.floor((width + gap) / (148 + gap)))
            readonly property int filesTop: 210

            Text {
                textFormat: Text.StyledText
                text: "<b>Folders</b>  <font color='#9a97ab'>" + win.folders.length + "</font>"
                font.pixelSize: 15; color: "#16132a"
            }
            Row {
                y: 30; spacing: 14
                Repeater {
                    model: win.folders
                    delegate: GlassFolder {
                        required property var modelData
                        required property int index
                        objectName: "folder" + index
                        label: modelData.name
                        count: "folder"
                        forcedHover: script.hoverFolder === index
                    }
                }
            }

            Text {
                y: stage.filesTop - 34
                textFormat: Text.StyledText
                text: win.byKind ? "<b>By kind</b>  <font color='#9a97ab'>" + win.kindGroups.length + "</font>"
                                 : "<b>Files</b>  <font color='#9a97ab'>" + win.files.length + "</font>"
                font.pixelSize: 15; color: "#16132a"
            }

            // Kind folders grow in when grouping by kind.
            Repeater {
                id: kindFolders
                model: win.kindGroups
                delegate: GlassFolder {
                    required property var modelData
                    required property int index
                    x: (index % stage.kindColumns) * (148 + stage.gap)
                    y: stage.filesTop + Math.floor(index / stage.kindColumns) * 160
                    deep: win.deep[modelData.kind]
                    light: win.light[modelData.kind]
                    label: win.labels[modelData.kind]
                    count: modelData.count + (modelData.count === 1 ? " file" : " files")
                    size: win.human(modelData.bytes)
                    opacity: win.byKind ? 1 : 0
                    scale: win.byKind ? 1 : 0.55
                    visible: opacity > 0.01
                    Behavior on opacity { SequentialAnimation {
                        PauseAnimation { duration: win.byKind ? 260 + index * 60 : 0 }
                        NumberAnimation { duration: 260 } } }
                    Behavior on scale { SequentialAnimation {
                        PauseAnimation { duration: win.byKind ? 260 + index * 60 : 0 }
                        SpringAnimation { spring: 3.2; damping: 0.22; epsilon: 0.005 } } }
                }
            }

            // File cards: in a grid, or flown into their kind folder.
            Repeater {
                model: win.files
                delegate: FileCard {
                    required property var modelData
                    required property int index
                    readonly property int kindIndex: win.kindGroups.findIndex(g => g.kind === modelData.kind)
                    readonly property real gridX: (index % stage.columns) * (stage.cardW + stage.gap)
                    readonly property real gridY: stage.filesTop + Math.floor(index / stage.columns) * (stage.cardH + stage.gap)
                    readonly property real kindX: (kindIndex % stage.kindColumns) * (148 + stage.gap) + 74 - width / 2
                    readonly property real kindY: stage.filesTop + Math.floor(kindIndex / stage.kindColumns) * 160 + 50 - height / 2
                    readonly property int delay: Math.min(index * 22, 420)
                    name: modelData.name
                    meta: win.labels[modelData.kind] + " · " + win.human(modelData.size)
                    badge: (modelData.name.split(".").pop() || "FILE").toUpperCase().slice(0, 4)
                    tint: win.deep[modelData.kind]
                    forcedHover: script.hoverCard === index
                    x: win.byKind ? kindX : gridX
                    y: win.byKind ? kindY : gridY
                    scale: win.byKind ? 0.18 : 1
                    opacity: win.byKind ? 0 : 1
                    visible: opacity > 0.01
                    z: 1
                    Behavior on x { SequentialAnimation { PauseAnimation { duration: delay } NumberAnimation { duration: 560; easing.type: Easing.InOutCubic } } }
                    Behavior on y { SequentialAnimation { PauseAnimation { duration: delay } NumberAnimation { duration: 560; easing.type: Easing.InOutCubic } } }
                    Behavior on scale { SequentialAnimation { PauseAnimation { duration: delay } NumberAnimation { duration: 560; easing.type: Easing.InOutCubic } } }
                    Behavior on opacity { SequentialAnimation { PauseAnimation { duration: delay + (win.byKind ? 380 : 0) } NumberAnimation { duration: 200 } } }
                }
            }
        }
    }

    // --- right pane: a list that collapses and reflows ---------------------------------
    Rectangle {
        id: right
        x: parent.width - 300; y: 0; width: 300; height: parent.height
        color: "#f8f7fb"
        Rectangle { width: 1; height: parent.height; color: Qt.rgba(0.09, 0.07, 0.16, 0.08) }

        Text { x: 20; y: 24; text: "Free Up Space"; font.pixelSize: 17; font.weight: Font.Bold; color: "#16132a" }
        Text { x: 20; y: 50; text: "Delete removes the top row · Ctrl+Z puts it back"; font.pixelSize: 11; color: "#8a879a" }

        ListView {
            id: clean
            x: 14; y: 80; width: parent.width - 28; height: 420
            spacing: 8
            interactive: false
            property var removed: []
            model: ListModel {
                id: cleanModel
                ListElement { label: "npm download cache"; size: "1.1 GB"; tint: "#2c6cd6" }
                ListElement { label: "pip cache"; size: "640 MB"; tint: "#2c8a57" }
                ListElement { label: "Cargo registry"; size: "420 MB"; tint: "#dc7409" }
                ListElement { label: "Go build cache"; size: "260 MB"; tint: "#13778a" }
                ListElement { label: "Old screen recordings"; size: "200 MB"; tint: "#be3d77" }
                ListElement { label: "Old downloads"; size: "1.2 GB"; tint: "#7a7d86" }
            }
            function removeFirst() {
                if (cleanModel.count === 0) return
                const item = cleanModel.get(0)
                removed.push({ label: item.label, size: item.size, tint: item.tint })
                toast.show("Moved “" + item.label + "” to the Trash")
                cleanModel.remove(0)
            }
            function undo() {
                if (removed.length === 0) return
                cleanModel.insert(0, removed.pop())
                toast.show("Put back")
            }
            delegate: Rectangle {
                required property string label
                required property string size
                required property string tint
                width: clean.width; height: 54; radius: 12
                color: "white"
                border.color: Qt.rgba(0.09, 0.07, 0.16, 0.08)
                Accessible.role: Accessible.ListItem
                Accessible.name: label
                Rectangle { x: 12; anchors.verticalCenter: parent.verticalCenter; width: 10; height: 10; radius: 5; color: parent.tint }
                Text { x: 32; y: 10; text: parent.label; font.pixelSize: 13; color: "#16132a" }
                Text { x: 32; y: 29; text: parent.size; font.pixelSize: 11; color: "#5d5a72" }
            }
            remove: Transition {
                ParallelAnimation {
                    NumberAnimation { property: "opacity"; to: 0; duration: 240; easing.type: Easing.OutCubic }
                    NumberAnimation { property: "scale"; to: 0.9; duration: 240; easing.type: Easing.OutCubic }
                    NumberAnimation { property: "x"; to: 40; duration: 240; easing.type: Easing.OutCubic }
                }
            }
            displaced: Transition {
                SpringAnimation { properties: "y"; spring: 4; damping: 0.35 }
            }
            add: Transition {
                ParallelAnimation {
                    NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 260 }
                    NumberAnimation { property: "scale"; from: 0.85; to: 1; duration: 320; easing.type: Easing.OutBack }
                }
            }
        }
    }

    // --- toast ---------------------------------------------------------------------
    Rectangle {
        id: toast
        property bool shown: false
        property alias text: toastText.text
        function show(message) { text = message; shown = true; hideTimer.restart() }
        width: toastText.implicitWidth + 120; height: 44; radius: 22
        color: "#26233a"
        x: content.x + (content.width - width) / 2
        y: shown ? win.height - 70 : win.height + 20
        Behavior on y { SpringAnimation { spring: 4; damping: 0.32 } }
        Accessible.role: Accessible.AlertMessage
        Accessible.name: toastText.text
        Text { id: toastText; x: 20; anchors.verticalCenter: parent.verticalCenter; color: "white"; font.pixelSize: 13 }
        Rectangle {
            anchors.right: parent.right; anchors.rightMargin: 8; anchors.verticalCenter: parent.verticalCenter
            width: 64; height: 30; radius: 15; color: Qt.rgba(1, 1, 1, 0.14)
            Text { anchors.centerIn: parent; text: "Undo"; color: "white"; font.pixelSize: 13; font.weight: Font.DemiBold }
            TapHandler { onTapped: clean.undo() }
        }
        Timer { id: hideTimer; interval: 2600; onTriggered: toast.shown = false }
    }

    // --- scripted demo and frame capture (STEWARD_CAPTURE_DIR) ------------------------------
    QtObject {
        id: script
        property int frame: 0
        property int hoverFolder: -1
        property int hoverCard: -1
    }
    Timer {
        interval: 40
        repeat: true
        running: lister.capture !== "" && lister.status === "ready"
        onTriggered: {
            const n = script.frame++
            if (n === 10) { script.hoverFolder = 0; script.hoverCard = 1 }
            if (n === 28) { script.hoverFolder = -1; script.hoverCard = -1 }
            if (n === 36) win.byKind = true
            if (n === 90) win.byKind = false
            if (n === 140) clean.removeFirst()
            if (n === 160) clean.removeFirst()
            if (n === 185) clean.undo()
            if (n === 230) Qt.quit()
            const path = lister.capture + "/f" + String(n).padStart(4, "0") + ".png"
            win.contentItem.grabToImage(result => result.saveToFile(path))
        }
    }
}
