import QtQuick
import QtQuick.Controls.Basic

// Find anything (Ctrl+K, P3-T07, P3-T09; reference: search.png). The field
// types the query; Down moves into the results, where the arrows move,
// Space peeks, Enter opens and Ctrl+Enter shows the item in its folder.
FocusScope {
    id: root
    property var model: ({ open: false, sections: [], chips: [] })
    anchors.fill: parent
    z: 90
    visible: !!model.open

    function focusField() {
        field.forceActiveFocus()
        field.selectAll()
    }
    function keys(event, inField) {
        const ctrl = (event.modifiers & Qt.ControlModifier) !== 0
        if (event.key === Qt.Key_Escape) {
            p.activate("win.search-close", "")
        } else if (event.key === Qt.Key_Down || (!inField && event.key === Qt.Key_Right)) {
            p.activate("win.search-move", "down")
            results.forceActiveFocus()
        } else if (event.key === Qt.Key_Up || (!inField && event.key === Qt.Key_Left)) {
            if (!inField && root.model.selected === 0) field.forceActiveFocus()
            else p.activate("win.search-move", "up")
        } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            p.activate(ctrl ? "win.search-reveal" : "win.search-open", "")
        } else if (!inField && event.key === Qt.Key_Space) {
            p.activate("win.search-peek", "")
        } else if (!inField && event.text.length > 0 && !ctrl) {
            // Typing in the results goes back to the field.
            field.forceActiveFocus()
            field.insert(field.text.length, event.text)
        } else {
            return
        }
        event.accepted = true
    }

    Rectangle {
        anchors.fill: parent
        color: Qt.rgba(0.05, 0.04, 0.1, Theme.dark ? 0.5 : 0.22)
        MouseArea { anchors.fill: parent; onClicked: p.activate("win.search-close", "") }
    }

    // The query field.
    Rectangle {
        id: fieldBox
        anchors.horizontalCenter: parent.horizontalCenter
        y: 40
        width: Math.min(760, parent.width - 80)
        height: 64
        radius: 32
        color: Theme.card
        border.color: Theme.line
        layer.enabled: Theme.effects
        layer.samplerName: "tex"
        layer.effect: FxShadow { shadowOpacity: 0.18; shadowBlur: 0.8; shadowVerticalOffset: 6 }
        TextField {
            id: field
            anchors.fill: parent
            anchors.leftMargin: 26
            anchors.rightMargin: 56
            background: null
            color: Theme.ink
            font.pixelSize: 24
            placeholderText: p.t("Find folders and files")
            placeholderTextColor: Theme.faint
            Accessible.name: p.t("Find anything")
            onTextChanged: p.searchquery(text)
            Keys.onPressed: event => root.keys(event, true)
        }
        Icon { anchors.right: parent.right; anchors.rightMargin: 24; anchors.verticalCenter: parent.verticalCenter
               name: "search"; size: 22; color: Theme.muted }
    }

    // The results.
    Rectangle {
        id: panel
        visible: (root.model.ran || "") !== "" || !root.model.running
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: fieldBox.bottom
        anchors.topMargin: 16
        width: fieldBox.width
        height: Math.min(parent.height - y - 30, column.implicitHeight + footer.height + 24)
        radius: 20
        color: Theme.card
        border.color: Theme.line
        clip: true
        layer.enabled: Theme.effects
        layer.samplerName: "tex"
        layer.effect: FxShadow { shadowOpacity: 0.18; shadowBlur: 0.8; shadowVerticalOffset: 6 }
        Accessible.role: Accessible.List
        Accessible.name: p.t("Search results")

        Flickable {
            id: results
            anchors.top: parent.top
            anchors.bottom: footer.top
            width: parent.width
            contentHeight: column.implicitHeight
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            focus: true
            activeFocusOnTab: true
            Keys.onPressed: event => root.keys(event, false)

            Column {
                id: column
                width: parent.width
                padding: 20
                spacing: 14

                // Not running: say so, offer to start it (E3-12).
                Column {
                    visible: !root.model.running
                    spacing: 10
                    Text { text: root.model.status || ""; color: Theme.ink; font.pixelSize: 15 }
                    PillButton { text: p.t("Start Indexing"); primary: true; onClicked: p.activate("win.start-indexer", "") }
                }

                Flow {
                    width: column.width - 40
                    spacing: 8
                    visible: (root.model.chips || []).length > 0
                    Repeater {
                        model: root.model.chips || []
                        delegate: Rectangle {
                            id: chip
                            required property var modelData
                            width: chipText.implicitWidth + (modelData.id !== "all" ? 40 : 26)
                            height: 32; radius: 9
                            color: modelData.active ? Theme.alpha(Theme.accent, 0.12) : Theme.pane
                            border.color: modelData.active ? Theme.accent : Theme.line
                            Accessible.role: Accessible.Button
                            Accessible.name: modelData.label + " " + modelData.count
                            Accessible.onPressAction: p.activate("win.search-kind", modelData.id)
                            Rectangle { visible: chip.modelData.id !== "all"; x: 13; anchors.verticalCenter: parent.verticalCenter
                                        width: 9; height: 9; radius: 2.5; color: Theme.deep(chip.modelData.id) }
                            Text {
                                id: chipText
                                x: chip.modelData.id !== "all" ? 28 : 13
                                anchors.verticalCenter: parent.verticalCenter
                                textFormat: Text.StyledText
                                text: "<b>" + chip.modelData.label + "</b>&nbsp;&nbsp;<font color='" + Theme.faint + "'>"
                                      + chip.modelData.count + "</font>"
                                color: Theme.ink; font.pixelSize: 14
                            }
                            MouseArea { anchors.fill: parent; onClicked: p.activate("win.search-kind", chip.modelData.id) }
                        }
                    }
                }

                Text {
                    visible: root.model.running && (root.model.ran || "") !== "" && (root.model.count || 0) === 0
                    text: p.t("Nothing is called that.")
                    color: Theme.muted; font.pixelSize: 15
                }

                Repeater {
                    model: root.model.sections || []
                    delegate: Column {
                        id: section
                        required property var modelData
                        required property int index
                        width: column.width - 40
                        spacing: 6
                        Text {
                            text: section.modelData.title.toUpperCase()
                            color: Theme.muted; font.pixelSize: 12; font.weight: Font.DemiBold; font.letterSpacing: 0.6
                            topPadding: 4; bottomPadding: 2
                        }
                        // Images and videos as tiles; the rest as rows.
                        Flow {
                            width: parent.width
                            spacing: 10
                            visible: section.index >= 0 && section.modelData.title.indexOf("video") >= 0
                            Repeater {
                                model: parent.visible ? section.modelData.items : []
                                delegate: SearchTile { required property var modelData; item: modelData }
                            }
                        }
                        Repeater {
                            model: section.modelData.title.indexOf("video") >= 0 ? [] : section.modelData.items
                            delegate: SearchRow { required property var modelData; item: modelData; width: section.width }
                        }
                    }
                }
            }
        }

        Rectangle {
            id: footer
            anchors.bottom: parent.bottom
            width: parent.width
            height: 44
            color: Theme.card
            Rectangle { width: parent.width; height: 1; color: Theme.line }
            Text {
                x: 20; anchors.verticalCenter: parent.verticalCenter
                text: root.model.summary || ""
                color: Theme.muted; font.pixelSize: 13
                width: parent.width - hints.width - 40; elide: Text.ElideRight
            }
            Text {
                id: hints
                anchors.right: parent.right; anchors.rightMargin: 20
                anchors.verticalCenter: parent.verticalCenter
                text: p.t("↑↓ move   ↵ open   Space peek   Ctrl+↵ show in folder")
                color: Theme.muted; font.pixelSize: 13
            }
        }
    }

    component SearchRow: Rectangle {
        id: row
        property var item
        height: 56
        radius: 10
        color: item.selected ? Theme.alpha(Theme.accent, 0.14) : (rowHover.hovered ? Theme.hover : "transparent")
        Accessible.role: Accessible.ListItem
        Accessible.name: item.name
        Accessible.selected: item.selected
        Accessible.onPressAction: { p.activate("win.search-select", String(item.index)); p.activate("win.search-open", "") }
        Rectangle {
            x: 10; anchors.verticalCenter: parent.verticalCenter
            width: 40; height: 40; radius: 10
            color: row.item.dir ? Theme.alpha(Theme.deep("folders"), 0.16) : Theme.deep(row.item.kind)
            Icon { visible: !!row.item.dir; anchors.centerIn: parent; name: "folder"; size: 20; color: Theme.deep("folders") }
            Text { visible: !row.item.dir; anchors.centerIn: parent; text: row.item.badge; color: "white"
                   font.pixelSize: 10; font.weight: Font.Black }
        }
        Column {
            x: 62; anchors.verticalCenter: parent.verticalCenter
            width: parent.width - x - date.width - 20
            Text { width: parent.width; text: row.item.name; elide: Text.ElideMiddle; color: Theme.ink
                   font.pixelSize: 15; font.weight: Font.DemiBold }
            Text { width: parent.width; text: row.item.meta; elide: Text.ElideMiddle; color: Theme.muted; font.pixelSize: 13 }
        }
        Text { id: date; anchors.right: parent.right; anchors.rightMargin: 14; anchors.verticalCenter: parent.verticalCenter
               text: row.item.dir ? "" : row.item.date; color: Theme.muted; font.pixelSize: 13 }
        HoverHandler { id: rowHover }
        MouseArea {
            anchors.fill: parent
            onClicked: p.activate("win.search-select", String(row.item.index))
            onDoubleClicked: p.activate("win.search-open", "")
        }
    }

    component SearchTile: Rectangle {
        id: tile
        property var item
        width: 200; height: 176
        radius: 12
        color: Theme.card
        border.color: item.selected ? Theme.accent : Theme.line
        border.width: item.selected ? 2 : 1
        Accessible.role: Accessible.ListItem
        Accessible.name: item.name
        Accessible.selected: item.selected
        Rectangle {
            x: 6; y: 6; width: parent.width - 12; height: 116; radius: 8
            color: Theme.deep(tile.item.kind)
            clip: true
            Icon { anchors.centerIn: parent; name: tile.item.kind === "videos" ? "videos" : "pictures"; size: 34
                   color: Qt.rgba(1, 1, 1, 0.8) }
            Image { anchors.fill: parent; source: tile.item.thumb ? "file://" + tile.item.thumb : ""
                    fillMode: Image.PreserveAspectCrop; asynchronous: true }
        }
        Column {
            x: 8; y: 128; width: parent.width - 16
            Text { width: parent.width; text: tile.item.name; elide: Text.ElideRight; color: Theme.ink
                   font.pixelSize: 14; font.weight: Font.DemiBold }
            Text { width: parent.width; text: tile.item.where; elide: Text.ElideMiddle; color: Theme.muted; font.pixelSize: 12 }
        }
        MouseArea {
            anchors.fill: parent
            onClicked: p.activate("win.search-select", String(tile.item.index))
            onDoubleClicked: p.activate("win.search-open", "")
        }
    }
}
