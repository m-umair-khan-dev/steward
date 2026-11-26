import QtQuick
import QtQuick.Window
import QtQuick.Controls.Basic

// The Shelf (P8-T01; DESIGN.md §4.4): a small window of its own, summoned
// from anywhere. Numbered pinned folders (drop to move, Ctrl to copy; keys
// 1 to 9 open them), Holding (remembers files without moving them; drag
// them out to any app), the latest downloads and quick access. The
// compositor places it; the optional Shell extension may pin it top-centre.
Window {
    id: shelf
    property var model: ({ open: false })
    readonly property var pins: model.pins || []
    readonly property var held: model.held || []
    readonly property var latest: model.latest || []
    readonly property var quick: model.quick || []

    title: p.t("Shelf")
    width: 760
    height: column.implicitHeight + 36
    minimumWidth: 560
    flags: Qt.Window | Qt.FramelessWindowHint
    color: "transparent"
    visible: !!model.open
    onVisibleChanged: if (visible) { requestActivate(); body.forceActiveFocus() }
    onClosing: close => p.activate("win.shelf-close", "")

    Rectangle {
        id: body
        anchors.fill: parent
        anchors.margins: 6
        radius: 22
        color: Theme.pane
        border.color: Theme.line
        focus: true
        Accessible.role: Accessible.Pane
        Accessible.name: p.t("Shelf")
        Keys.onPressed: event => {
            if (event.key === Qt.Key_Escape) {
                p.activate("win.shelf-close", "")
                event.accepted = true
            } else if (event.key >= Qt.Key_1 && event.key <= Qt.Key_9 && !event.modifiers) {
                p.activate("win.shelf-open-pin", String(event.key - Qt.Key_0))
                event.accepted = true
            }
        }
        // Move the frameless window by its background.
        DragHandler { target: null; onActiveChanged: if (active) shelf.startSystemMove() }

        Column {
            id: column
            x: 20; y: 16
            width: parent.width - 40
            spacing: 14

            // --- title and result line ----------------------------------------
            Item {
                width: parent.width; height: 30
                Row {
                    spacing: 10
                    anchors.verticalCenter: parent.verticalCenter
                    Icon { name: "box"; size: 18; color: Theme.accentText; anchors.verticalCenter: parent.verticalCenter }
                    Text { text: p.t("Shelf"); color: Theme.ink; font.pixelSize: 16; font.weight: Font.Bold
                           anchors.verticalCenter: parent.verticalCenter
                           Accessible.role: Accessible.Heading; Accessible.name: text }
                }
                Row {
                    anchors.right: closeButton.left; anchors.rightMargin: 10
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 8
                    visible: !!shelf.model.result
                    Text { id: resultText; text: (shelf.model.result || {}).text || ""; color: Theme.ink; font.pixelSize: 13
                           anchors.verticalCenter: parent.verticalCenter; elide: Text.ElideMiddle
                           width: Math.min(implicitWidth, 360)
                           Accessible.role: Accessible.StaticText; Accessible.name: text }
                    PillButton { visible: !!(shelf.model.result || {}).undo; implicitHeight: 26; text: p.t("Undo")
                                 onClicked: p.activate("win.shelf-undo", "") }
                }
                IconButton { id: closeButton; anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                             width: 28; height: 28; iconSize: 13; flat: true; icon: "close"; label: p.t("Close the Shelf")
                             onClicked: p.activate("win.shelf-close", "") }
            }

            // --- pinned folders -------------------------------------------------
            Row {
                width: parent.width
                spacing: 10
                Accessible.role: Accessible.List
                Accessible.name: p.t("Pinned folders")
                Repeater {
                    model: shelf.pins
                    delegate: Rectangle {
                        id: pin
                        required property var modelData
                        width: Math.min(150, (column.width - 10 * (Math.max(shelf.pins.length, 1) - 1)) / Math.max(shelf.pins.length, 1))
                        height: 96
                        radius: 16
                        color: drop.containsDrag ? Theme.alpha(Theme.accent, 0.16) : (hover.hovered ? Theme.hover : Theme.card)
                        border.color: drop.containsDrag ? Theme.accent : Theme.line
                        border.width: drop.containsDrag ? 2 : 1
                        Accessible.role: Accessible.Button
                        Accessible.name: modelData.n + ": " + modelData.name
                        Accessible.description: modelData.count === null ? "" : modelData.count + p.t(" items")
                        Accessible.onPressAction: p.activate("win.shelf-open-pin", String(modelData.n))
                        Rectangle {
                            x: 10; y: 10; width: 20; height: 20; radius: 6
                            color: Theme.alpha(Theme.accent, 0.14)
                            Text { anchors.centerIn: parent; text: pin.modelData.n; color: Theme.accentText
                                   font.pixelSize: 12; font.weight: Font.Bold }
                        }
                        MiniFolder { anchors.horizontalCenter: parent.horizontalCenter; y: 16; size: 34; kind: "folders" }
                        Column {
                            anchors.horizontalCenter: parent.horizontalCenter
                            y: 52
                            width: parent.width - 16
                            Text { width: parent.width; horizontalAlignment: Text.AlignHCenter; elide: Text.ElideMiddle
                                   text: pin.modelData.name; color: Theme.ink; font.pixelSize: 13; font.weight: Font.DemiBold }
                            Text { width: parent.width; horizontalAlignment: Text.AlignHCenter
                                   text: pin.modelData.count === null ? "" : pin.modelData.count + p.t(" items")
                                   color: Theme.faint; font.pixelSize: 11 }
                        }
                        HoverHandler { id: hover; cursorShape: Qt.PointingHandCursor }
                        TapHandler { onTapped: p.activate("win.shelf-open-pin", String(pin.modelData.n)) }
                        DropArea {
                            id: drop
                            anchors.fill: parent
                            keys: ["text/uri-list"]
                            onDropped: drop => {
                                if (!drop.hasUrls) return
                                const uris = drop.urls.map(u => u.toString()).join("\n")
                                // Qt proposes Copy while Ctrl is held.
                                const how = drop.proposedAction === Qt.CopyAction ? "copy" : "move"
                                p.activate("win.shelf-drop", pin.modelData.n + "|" + how + "|" + uris)
                                drop.acceptProposedAction()
                            }
                        }
                    }
                }
                Text {
                    visible: shelf.pins.length === 0
                    text: p.t("Pin a folder from its menu to see it here.")
                    color: Theme.faint; font.pixelSize: 13
                }
            }

            // --- Holding -------------------------------------------------------
            Rectangle {
                width: parent.width
                height: Math.max(64, heldFlow.implicitHeight + 46)
                radius: 16
                color: heldDrop.containsDrag ? Theme.alpha(Theme.accent, 0.10) : Theme.sunken
                border.color: heldDrop.containsDrag ? Theme.accent : "transparent"
                border.width: 2
                Accessible.role: Accessible.Grouping
                Accessible.name: p.t("Holding")
                Row {
                    x: 14; y: 10; spacing: 8
                    Text { text: p.t("Holding"); color: Theme.muted; font.pixelSize: 12; font.weight: Font.DemiBold
                           font.capitalization: Font.AllUppercase }
                    Text { text: shelf.held.length ? shelf.held.length : ""; color: Theme.faint; font.pixelSize: 12 }
                }
                PillButton { anchors.right: parent.right; anchors.rightMargin: 10; y: 6; implicitHeight: 24
                             visible: shelf.held.length > 0; text: p.t("Clear")
                             onClicked: p.activate("win.shelf-clear", "") }
                Text {
                    x: 14; y: 32
                    visible: shelf.held.length === 0
                    text: p.t("Drop files here to keep them at hand. Nothing moves.")
                    color: Theme.faint; font.pixelSize: 13
                }
                Flow {
                    id: heldFlow
                    x: 14; y: 34
                    width: parent.width - 28
                    spacing: 8
                    Accessible.role: Accessible.List
                    Accessible.name: p.t("Held items")
                    Repeater {
                        model: shelf.held
                        delegate: Chip {
                            required property var modelData
                            item: modelData
                            removable: true
                            onRemove: p.activate("win.shelf-unhold", String(modelData.id))
                        }
                    }
                }
                DropArea {
                    id: heldDrop
                    anchors.fill: parent
                    keys: ["text/uri-list"]
                    onDropped: drop => {
                        if (!drop.hasUrls) return
                        p.activate("win.shelf-hold", drop.urls.map(u => u.toString()).join("\n"))
                        drop.accept(Qt.LinkAction)
                    }
                }
            }

            // --- latest downloads and quick access ------------------------------
            Item {
                width: parent.width
                height: Math.max(latestColumn.implicitHeight, quickColumn.implicitHeight)
                Column {
                    id: latestColumn
                    width: parent.width - quickColumn.width - 20
                    spacing: 6
                    Text { text: p.t("Latest downloads"); color: Theme.muted; font.pixelSize: 12; font.weight: Font.DemiBold
                           font.capitalization: Font.AllUppercase }
                    Text { visible: shelf.latest.length === 0; text: p.t("Nothing downloaded yet.")
                           color: Theme.faint; font.pixelSize: 13 }
                    Flow {
                        width: parent.width
                        spacing: 8
                        Accessible.role: Accessible.List
                        Accessible.name: p.t("Latest downloads")
                        Repeater {
                            model: shelf.latest
                            delegate: Chip { required property var modelData; item: modelData }
                        }
                    }
                }
                Column {
                    id: quickColumn
                    anchors.right: parent.right
                    width: 170
                    spacing: 4
                    Text { text: p.t("Quick access"); color: Theme.muted; font.pixelSize: 12; font.weight: Font.DemiBold
                           font.capitalization: Font.AllUppercase }
                    Repeater {
                        model: shelf.quick
                        delegate: Rectangle {
                            required property var modelData
                            width: quickColumn.width; height: 28; radius: 8
                            color: qHover.hovered ? Theme.hover : "transparent"
                            Accessible.role: Accessible.Button
                            Accessible.name: modelData.label
                            Accessible.onPressAction: p.activate("win.shelf-open", modelData.path)
                            Row {
                                x: 6; anchors.verticalCenter: parent.verticalCenter; spacing: 8
                                Icon { name: parent.parent.modelData.icon; size: 16; color: Theme.accentText
                                       anchors.verticalCenter: parent.verticalCenter }
                                Text { text: parent.parent.modelData.label; color: Theme.ink; font.pixelSize: 13
                                       anchors.verticalCenter: parent.verticalCenter }
                            }
                            HoverHandler { id: qHover; cursorShape: Qt.PointingHandCursor }
                            TapHandler { onTapped: p.activate("win.shelf-open", parent.modelData.path) }
                        }
                    }
                }
            }
        }
    }

    // A file as a chip: click shows it in its folder; drag it out to any app.
    component Chip: Rectangle {
        id: chip
        property var item
        property bool removable: false
        signal remove()
        height: 32
        width: Math.min(220, chipRow.implicitWidth + 16)
        radius: 10
        color: chipHover.hovered ? Theme.hover : Theme.card
        border.color: Theme.line
        opacity: item.exists === false ? 0.5 : 1
        Accessible.role: Accessible.ListItem
        Accessible.name: item.name
        Accessible.onPressAction: p.activate("win.shelf-open", item.path)
        Row {
            id: chipRow
            x: 8; anchors.verticalCenter: parent.verticalCenter
            spacing: 6
            Rectangle {
                width: 20; height: 20; radius: 5
                anchors.verticalCenter: parent.verticalCenter
                color: chip.item.dir ? Theme.alpha(Theme.deep("folders"), 0.16) : Theme.badge(chip.item.kind || "other")
                Text { visible: !chip.item.dir; anchors.centerIn: parent; text: chip.item.badge || ""; color: Theme.onAccent
                       font.pixelSize: 7; font.weight: Font.Black }
                Icon { visible: !!chip.item.dir; anchors.centerIn: parent; name: "folder"; size: 12; color: Theme.deep("folders") }
            }
            Text { text: chip.item.name; color: Theme.ink; font.pixelSize: 12; elide: Text.ElideMiddle
                   width: Math.min(implicitWidth, chip.removable ? 140 : 160); anchors.verticalCenter: parent.verticalCenter }
            IconButton { visible: chip.removable; width: 20; height: 20; iconSize: 10; flat: true; icon: "close"
                         anchors.verticalCenter: parent.verticalCenter
                         label: p.t("Take out of Holding: ") + chip.item.name
                         onClicked: chip.remove() }
        }
        HoverHandler { id: chipHover }
        MouseArea {
            id: dragArea
            anchors.fill: parent
            anchors.rightMargin: chip.removable ? 26 : 0
            drag.target: dragProxy
            drag.threshold: 8
            onClicked: p.activate("win.shelf-open", chip.item.path)
            onReleased: { dragProxy.x = 0; dragProxy.y = 0 }
        }
        Item {
            id: dragProxy
            Drag.active: dragArea.drag.active
            Drag.dragType: Drag.Automatic
            Drag.supportedActions: Qt.CopyAction | Qt.MoveAction | Qt.LinkAction
            Drag.proposedAction: Qt.CopyAction
            Drag.mimeData: ({ "text/uri-list": chip.item.uri || "" })
        }
    }
}
