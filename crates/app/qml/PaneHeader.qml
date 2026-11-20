import QtQuick

// A pane's header in dual pane: folder icon, the last few path segments,
// the selection summary, and an accent line under the active pane.
Rectangle {
    id: root
    property var crumbs: []
    property string summary: ""
    property bool active: false
    signal crumb(string path)
    height: 50
    color: Theme.pane
    Accessible.role: Accessible.Grouping
    Accessible.name: active ? "Active pane" : "Pane"

    Icon { x: 18; anchors.verticalCenter: parent.verticalCenter; name: "folder"; size: 20; color: Theme.deep("folders") }
    Row {
        x: 48
        anchors.verticalCenter: parent.verticalCenter
        spacing: 4
        Repeater {
            model: root.crumbs
            delegate: Row {
                required property var modelData
                required property int index
                spacing: 4
                Text { visible: parent.index > 0; text: "›"; color: Theme.faint; font.pixelSize: 15 }
                Text {
                    text: parent.modelData.label
                    color: parent.index === root.crumbs.length - 1 ? Theme.ink : Theme.muted
                    font.pixelSize: 15
                    font.weight: parent.index === root.crumbs.length - 1 ? Font.Bold : Font.Normal
                    MouseArea { anchors.fill: parent; onClicked: root.crumb(parent.parent.modelData.path) }
                }
            }
        }
    }
    Text {
        anchors.right: parent.right; anchors.rightMargin: 18
        anchors.verticalCenter: parent.verticalCenter
        text: root.summary
        color: Theme.muted; font.pixelSize: 13
    }
    Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: root.active ? 3 : 1
                color: root.active ? Theme.accent : Theme.line }
}
