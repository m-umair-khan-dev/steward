import QtQuick

// Shown above the Trash's contents (P2-T09): put back and empty.
Item {
    id: root
    property bool hasSelection: false
    property bool empty: true
    height: 54
    Accessible.role: Accessible.ToolBar
    Accessible.name: "Trash"

    Text {
        x: 22
        anchors.verticalCenter: parent.verticalCenter
        text: "Items here can be put back where they were."
        color: Theme.muted
        font.pixelSize: 13
    }
    Row {
        anchors.right: parent.right
        anchors.rightMargin: 22
        anchors.verticalCenter: parent.verticalCenter
        spacing: 8
        PillButton { text: "Put Back"; enabled: root.hasSelection; onClicked: p.activate("win.restore", "") }
        PillButton { text: "Empty Trash…"; enabled: !root.empty; destructive: true; onClicked: p.activate("win.empty-trash", "") }
    }
}
