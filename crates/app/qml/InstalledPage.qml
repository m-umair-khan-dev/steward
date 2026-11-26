import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs

// Installed (P5-T09; reference: installed.png): how many packages from which
// tools, filters, and a table with why each one is there. Read-only:
// removing is a command to copy.
Rectangle {
    id: root
    property var model: ({ looking: true, tools: [], filters: [], packages: [] })
    color: Theme.pane
    Accessible.role: Accessible.Pane
    Accessible.name: p.t("Installed")

    Column {
        id: head
        x: 32; y: 22
        width: parent.width - 64
        spacing: 16
        // In a sandbox there is no list to sum up or filter (P9-T01).
        visible: !root.model.unavailable

        Rectangle {
            width: parent.width
            height: summary.implicitHeight + 44
            radius: 18; color: Theme.card; border.color: Theme.line
            Column {
                id: summary
                x: 24; y: 22
                width: parent.width - 48
                spacing: 14
                Item {
                    width: parent.width; height: titleRow.implicitHeight
                    Row {
                        id: titleRow
                        spacing: 12
                        Text { text: root.model.looking && !root.model.title ? p.t("Looking…") : (root.model.title || "")
                               color: Theme.ink; font.pixelSize: 30; font.weight: Font.Bold
                               Accessible.role: Accessible.Heading; Accessible.name: text }
                        Text { text: root.model.summary || ""; color: Theme.muted; font.pixelSize: 15
                               anchors.baseline: parent.children[0].baseline }
                    }
                    Row {
                        anchors.right: parent.right
                        spacing: 8
                        PillButton { text: p.t("Save List…"); enabled: !!root.model.title; onClicked: saveDialog.open() }
                        IconButton { icon: "redo"; label: p.t("Look again"); onClicked: p.activate("win.installed-refresh", "") }
                    }
                }
                // One bar, a part per tool.
                Row {
                    width: parent.width; height: 10
                    Repeater {
                        model: root.model.tools || []
                        delegate: Rectangle {
                            required property var modelData
                            required property int index
                            width: Math.max(4, summary.width * modelData.share)
                            height: 10
                            radius: 5
                            color: Theme.deep(modelData.color)
                        }
                    }
                }
                Flow {
                    width: parent.width
                    spacing: 10
                    Accessible.role: Accessible.List
                    Accessible.name: p.t("Tools")
                    Repeater {
                        model: root.model.tools || []
                        delegate: Rectangle {
                            id: chip
                            required property var modelData
                            height: 38; radius: 10
                            width: chipRow.implicitWidth + 24
                            color: modelData.selected ? Theme.alpha(Theme.accent, 0.16) : Theme.sunken
                            border.color: modelData.selected ? Theme.accent : "transparent"
                            Accessible.role: Accessible.Button
                            Accessible.name: modelData.name
                            Accessible.description: modelData.count + p.t(" packages, ") + modelData.bytesText
                            Accessible.onPressAction: p.activate("win.installed-tool", modelData.id)
                            Row {
                                id: chipRow
                                anchors.centerIn: parent
                                spacing: 8
                                Rectangle { width: 22; height: 22; radius: 6; color: Theme.badge(chip.modelData.color)
                                            anchors.verticalCenter: parent.verticalCenter
                                            Text { anchors.centerIn: parent; text: chip.modelData.name.substring(0, 2)
                                                   color: Theme.onAccent; font.pixelSize: 10; font.weight: Font.Bold } }
                                Text { text: chip.modelData.name; color: Theme.ink; font.pixelSize: 15; font.weight: Font.DemiBold
                                       anchors.verticalCenter: parent.verticalCenter }
                                Text { text: chip.modelData.count + " · " + chip.modelData.bytesText; color: Theme.muted
                                       font.pixelSize: 14; anchors.verticalCenter: parent.verticalCenter }
                            }
                            TapHandler { onTapped: p.activate("win.installed-tool", chip.modelData.id) }
                        }
                    }
                }
            }
        }

        Row {
            spacing: 10
            Accessible.role: Accessible.PageTabList
            Accessible.name: p.t("Filters")
            Repeater {
                model: root.model.filters || []
                delegate: Rectangle {
                    id: filter
                    required property var modelData
                    height: 36; radius: 18
                    width: filterRow.implicitWidth + 28
                    color: modelData.selected ? Theme.alpha(Theme.accent, 0.12) : Theme.sunken
                    border.color: modelData.selected ? Theme.accent : "transparent"
                    border.width: 1.5
                    Accessible.role: Accessible.PageTab
                    Accessible.name: modelData.label
                    Accessible.selected: modelData.selected
                    Accessible.onPressAction: p.activate("win.installed-filter", modelData.id)
                    Row {
                        id: filterRow
                        anchors.centerIn: parent
                        spacing: 6
                        Text { text: filter.modelData.label; color: Theme.ink; font.pixelSize: 15; font.weight: Font.DemiBold }
                        Text { text: filter.modelData.count; color: Theme.muted; font.pixelSize: 15 }
                    }
                    TapHandler { onTapped: p.activate("win.installed-filter", filter.modelData.id) }
                }
            }
        }
    }

    Rectangle {
        id: table
        anchors.top: head.bottom; anchors.topMargin: 16
        anchors.bottom: parent.bottom; anchors.bottomMargin: 20
        x: 32; width: parent.width - 64
        radius: 14; color: Theme.card; border.color: Theme.line
        clip: true
        visible: !root.model.unavailable

        Item {
            id: columns
            width: parent.width; height: 40
            Text { x: 18; anchors.verticalCenter: parent.verticalCenter; text: p.t("Name"); color: Theme.muted; font.pixelSize: 13; font.weight: Font.DemiBold }
            Text { x: parent.width * 0.55; anchors.verticalCenter: parent.verticalCenter; text: p.t("Tool"); color: Theme.muted; font.pixelSize: 13; font.weight: Font.DemiBold }
            Text { x: parent.width * 0.70; anchors.verticalCenter: parent.verticalCenter; text: p.t("Why it’s here"); color: Theme.muted; font.pixelSize: 13; font.weight: Font.DemiBold }
            Text { anchors.right: parent.right; anchors.rightMargin: 64; anchors.verticalCenter: parent.verticalCenter; text: p.t("Size"); color: Theme.muted; font.pixelSize: 13; font.weight: Font.DemiBold }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.line }
        }
        ListView {
            id: list
            anchors.top: columns.bottom
            anchors.bottom: parent.bottom
            width: parent.width
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            model: root.model.packages || []
            Accessible.role: Accessible.List
            Accessible.name: p.t("Packages")
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
            delegate: Rectangle {
                id: row
                required property var modelData
                width: list.width; height: 58
                color: rowHover.hovered ? Theme.hover : "transparent"
                Accessible.role: Accessible.ListItem
                Accessible.name: modelData.name
                Accessible.description: modelData.version + ", " + modelData.toolName + ", " + modelData.whyLabel
                HoverHandler { id: rowHover }
                Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.line }
                Rectangle {
                    x: 18; anchors.verticalCenter: parent.verticalCenter
                    width: 30; height: 30; radius: 8; color: Theme.badge(row.modelData.color)
                    Text { anchors.centerIn: parent; text: row.modelData.name.substring(0, 1).toUpperCase()
                           color: Theme.onAccent; font.pixelSize: 14; font.weight: Font.Bold }
                }
                Column {
                    x: 62; anchors.verticalCenter: parent.verticalCenter
                    width: parent.width * 0.55 - 70
                    spacing: 2
                    Row {
                        spacing: 6
                        Text { text: row.modelData.name; color: Theme.ink; font.pixelSize: 15; font.weight: Font.Bold }
                        Text { text: row.modelData.version; color: Theme.muted; font.pixelSize: 14 }
                    }
                    Text { width: parent.width; text: row.modelData.summary || row.modelData.date; color: Theme.faint
                           font.pixelSize: 12; elide: Text.ElideRight; visible: text !== "" }
                }
                Text { x: parent.width * 0.55; anchors.verticalCenter: parent.verticalCenter
                       text: row.modelData.toolName; color: Theme.muted; font.pixelSize: 14 }
                Rectangle {
                    x: parent.width * 0.70; anchors.verticalCenter: parent.verticalCenter
                    height: 24; radius: 12; width: whyText.implicitWidth + 20
                    color: row.modelData.why === "app" ? Theme.badge("documents")
                         : row.modelData.why === "you" ? Theme.accentFill : Theme.sunken
                    Text { id: whyText; anchors.centerIn: parent; text: row.modelData.whyLabel; font.pixelSize: 12
                           font.weight: Font.DemiBold; color: row.modelData.why === "came-with" ? Theme.muted : Theme.onAccent }
                }
                Text { anchors.right: parent.right; anchors.rightMargin: 64; anchors.verticalCenter: parent.verticalCenter
                       text: row.modelData.sizeText; color: Theme.ink; font.pixelSize: 14 }
                IconButton {
                    anchors.right: parent.right; anchors.rightMargin: 16; anchors.verticalCenter: parent.verticalCenter
                    width: 30; height: 30; iconSize: 15; flat: true; icon: "copy"
                    label: p.t("Copy the command that removes ") + row.modelData.name
                    onClicked: p.activate("win.installed-copy", row.modelData.key)
                }
            }
        }
        Text {
            anchors.centerIn: parent
            visible: !!root.model.looking && (root.model.packages || []).length === 0
            text: p.t("Asking apt, Snap, Flatpak and the other tools…")
            color: Theme.muted; font.pixelSize: 15
        }
    }

    // In a sandbox (P9-T01): why the list is not here.
    Column {
        anchors.centerIn: parent
        width: Math.min(parent.width - 80, 520)
        visible: !!root.model.unavailable
        spacing: 10
        Icon { anchors.horizontalCenter: parent.horizontalCenter; name: "lock"; size: 34; color: Theme.faint }
        Text { width: parent.width; horizontalAlignment: Text.AlignHCenter; wrapMode: Text.WordWrap
               text: root.model.unavailable || ""; color: Theme.muted; font.pixelSize: 15
               Accessible.role: Accessible.StaticText; Accessible.name: text }
    }

    FileDialog {
        id: saveDialog
        title: p.t("Save the List")
        fileMode: FileDialog.SaveFile
        onAccepted: p.activate("win.installed-save", selectedFile.toString())
    }
}
