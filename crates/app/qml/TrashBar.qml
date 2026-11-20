import QtQuick

// Shown above the Trash's contents (P2-T09): put back and empty.
Item {
    id: root
    property bool hasSelection: false
    property bool empty: true
    height: 54
    Accessible.role: Accessible.ToolBar
    Accessible.name: p.t("Trash")

    Text {
        x: 22
        anchors.verticalCenter: parent.verticalCenter
        text: p.t("Items here can be put back where they were.")
        color: Theme.muted
        font.pixelSize: 13
    }
    Row {
        anchors.right: parent.right
        anchors.rightMargin: 22
        anchors.verticalCenter: parent.verticalCenter
        spacing: 8
        PillButton { text: p.t("Put Back"); enabled: root.hasSelection; onClicked: p.activate("win.restore", "") }
        PillButton { text: p.t("Empty Trash…"); enabled: !root.empty; destructive: true; onClicked: p.activate("win.empty-trash", "") }
    }
}
