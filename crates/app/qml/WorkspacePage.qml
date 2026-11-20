import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs

// A workspace (P4-T08; reference: workspaces.png): banner, name and
// description, tabs, and its items with where each one lives. Nothing here
// moves files.
Rectangle {
    id: root
    property var model: ({ items: [], tabs: [] })
    property int locating: -1
    color: Theme.pane
    Accessible.role: Accessible.Pane
    Accessible.name: p.t("Workspace ") + (model.name || "")

    Rectangle {
        id: banner
        width: parent.width
        height: 130
        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop { position: 0; color: Theme.alpha(Theme.light(root.model.color || "folders"), 0.75) }
            GradientStop { position: 1; color: Theme.alpha(Theme.light(root.model.color || "folders"), 0.4) }
        }
    }
    MiniFolder { x: 40; y: banner.height - 52; size: 96; kind: root.model.color || "folders" }

    Column {
        id: head
        x: 40
        y: banner.height + 46
        width: parent.width - 80
        spacing: 6
        Item {
            width: parent.width; height: 44
            Text { text: root.model.name || ""; color: Theme.ink; font.pixelSize: 34; font.weight: Font.Bold
                   anchors.verticalCenter: parent.verticalCenter
                   Accessible.role: Accessible.Heading; Accessible.name: text }
            Row {
                anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                spacing: 8
                PillButton { text: p.t("+ Add Folder"); primary: true; onClicked: addDialog.open() }
                IconButton { icon: "more"; label: p.t("Workspace settings")
                             onClicked: p.activate("win.workspace-edit-ask", String(root.model.id)) }
            }
        }
        Text { width: parent.width; color: Theme.muted; font.pixelSize: 15; wrapMode: Text.WordWrap
               text: (root.model.description ? root.model.description + " · " : "") + (root.model.summary || "") }
        Row {
            spacing: 26
            topPadding: 14
            Repeater {
                model: root.model.tabs || []
                delegate: Item {
                    id: tab
                    required property var modelData
                    width: tabRow.implicitWidth; height: 38
                    readonly property bool chosen: root.model.tab === modelData.id
                    Accessible.role: Accessible.PageTab
                    Accessible.name: modelData.label
                    Accessible.selected: chosen
                    Accessible.onPressAction: p.activate("win.workspace-tab", modelData.id)
                    Row {
                        id: tabRow
                        spacing: 8
                        anchors.verticalCenter: parent.verticalCenter
                        Text { text: tab.modelData.label; font.pixelSize: 16; color: tab.chosen ? Theme.ink : Theme.muted
                               font.weight: tab.chosen ? Font.Bold : Font.Normal }
                        Rectangle { width: countText.implicitWidth + 14; height: 22; radius: 11; color: Theme.track
                                    anchors.verticalCenter: parent.verticalCenter
                                    Text { id: countText; anchors.centerIn: parent; text: tab.modelData.count
                                           color: Theme.muted; font.pixelSize: 12 } }
                    }
                    Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 3; radius: 1.5
                                visible: tab.chosen; color: Theme.deep(root.model.color || "folders") }
                    MouseArea { anchors.fill: parent; onClicked: p.activate("win.workspace-tab", tab.modelData.id) }
                }
            }
        }
        Rectangle { width: parent.width; height: 1; color: Theme.line }
    }

    Flickable {
        anchors.top: head.bottom; anchors.topMargin: 20
        anchors.bottom: parent.bottom
        x: 40
        width: parent.width - 80
        contentHeight: grid.implicitHeight + 30
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        Flow {
            id: grid
            width: parent.width
            spacing: 18
            Accessible.role: Accessible.List
            Accessible.name: p.t("Workspace items")
            Repeater {
                model: root.model.items || []
                delegate: Rectangle {
                    id: card
                    required property var modelData
                    readonly property bool here: modelData.state === "here"
                    width: 330; height: 120
                    radius: 16
                    color: Theme.card
                    border.color: Theme.line
                    opacity: here ? 1 : 0.6
                    Accessible.role: Accessible.ListItem
                    Accessible.name: modelData.name
                    MiniFolder { visible: card.modelData.dir; x: 22; y: 30; size: 66; kind: root.model.color || "folders" }
                    Rectangle {
                        visible: !card.modelData.dir
                        x: 30; y: 26; width: 50; height: 62; radius: 6
                        color: Theme.deep(card.modelData.kind)
                    }
                    Column {
                        x: 110; y: 18
                        width: parent.width - x - 16
                        spacing: 3
                        Text { width: parent.width; text: card.modelData.name; elide: Text.ElideMiddle; color: Theme.ink
                               font.pixelSize: 17; font.weight: Font.Bold }
                        Text { width: parent.width; text: card.modelData.label || card.modelData.meta; color: Theme.muted
                               font.pixelSize: 14; elide: Text.ElideRight }
                        Item {
                            width: parent.width; height: 20
                            Text { text: card.modelData.where; width: parent.width - date.width - 8; elide: Text.ElideMiddle
                                   color: Theme.faint; font.pixelSize: 13 }
                            Text { id: date; anchors.right: parent.right; text: card.modelData.date; color: Theme.faint
                                   font.pixelSize: 13 }
                        }
                        PillButton {
                            visible: card.modelData.state === "missing"
                            text: p.t("Locate…")
                            implicitHeight: 28
                            onClicked: { root.locating = card.modelData.id; locateDialog.open() }
                        }
                    }
                    MouseArea {
                        anchors.fill: parent
                        z: -1
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        onDoubleClicked: if (card.here) p.activate("win.go-to", card.modelData.dir ? card.modelData.path
                                                                   : card.modelData.path.replace(/\/[^/]*$/, ""))
                        onClicked: mouse => { if (mouse.button === Qt.RightButton) { itemMenu.item = card.modelData.id; itemMenu.popup() } }
                    }
                }
            }
        }
    }

    MenuPopup {
        id: itemMenu
        property int item: -1
        model: [{ text: p.t("Remove from Workspace"), action: "win.workspace-remove", param: String(itemMenu.item) }]
    }
    FolderDialog {
        id: addDialog
        title: p.t("Add a folder to “") + (root.model.name || "") + "”"
        onAccepted: p.activate("win.workspace-add", root.model.id + ":" + decodeURIComponent(selectedFolder.toString().replace("file://", "")))
    }
    FolderDialog {
        id: locateDialog
        title: p.t("Locate")
        onAccepted: p.activate("win.workspace-locate", root.locating + ":" + decodeURIComponent(selectedFolder.toString().replace("file://", "")))
    }
}
