import QtQuick
import QtQuick.Controls.Basic

// The right pane of dual pane (P4-T07; reference: dual-pane.png): its own
// folder, list and selection. Clicking it makes it the active pane.
Rectangle {
    id: root
    property var model: ({ items: [], crumbs: [] })
    color: Theme.pane
    Accessible.role: Accessible.Pane
    Accessible.name: p.t("Right pane")

    Rectangle { width: 1; height: parent.height; color: Theme.line }

    PaneHeader {
        id: header
        width: parent.width
        crumbs: root.model.crumbs || []
        summary: root.model.summary || ""
        active: !!root.model.rightActive
        onCrumb: path => p.activate("win.pane-go", path)
    }

    Item {
        id: columns
        anchors.top: header.bottom
        width: parent.width
        height: 34
        Text { x: 22; anchors.verticalCenter: parent.verticalCenter; text: p.t("Name"); color: Theme.faint
               font.pixelSize: 12; font.weight: Font.DemiBold }
        Text { x: parent.width * 0.58; anchors.verticalCenter: parent.verticalCenter; text: p.t("Modified")
               color: Theme.faint; font.pixelSize: 12; font.weight: Font.DemiBold }
        Text { anchors.right: parent.right; anchors.rightMargin: 22; anchors.verticalCenter: parent.verticalCenter
               text: p.t("Size"); color: Theme.faint; font.pixelSize: 12; font.weight: Font.DemiBold }
    }

    ListView {
        id: list
        anchors.top: columns.bottom
        anchors.bottom: parent.bottom
        width: parent.width
        clip: true
        model: root.model.items || []
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {}
        Accessible.role: Accessible.List
        Accessible.name: p.t("Right pane contents")
        delegate: Rectangle {
            id: line
            required property var modelData
            required property int index
            x: 10
            width: list.width - 20
            height: 36
            radius: 9
            color: modelData.selected ? Theme.alpha(Theme.accent, 0.22)
                                      : (index % 2 ? Theme.alpha(Theme.ink, 0.025) : "transparent")
            Accessible.role: Accessible.ListItem
            Accessible.name: modelData.name
            Accessible.selected: modelData.selected
            Icon {
                x: 12; anchors.verticalCenter: parent.verticalCenter; size: 16
                name: line.modelData.dir ? "folder" : "document"
                color: Theme.deep(line.modelData.kind)
            }
            Text { x: 40; width: parent.width * 0.58 - 48; anchors.verticalCenter: parent.verticalCenter
                   text: line.modelData.name; elide: Text.ElideMiddle; color: Theme.ink; font.pixelSize: 13 }
            Text { x: parent.width * 0.58 - 10; anchors.verticalCenter: parent.verticalCenter
                   text: line.modelData.modified; color: Theme.muted; font.pixelSize: 13 }
            Text { anchors.right: parent.right; anchors.rightMargin: 12; anchors.verticalCenter: parent.verticalCenter
                   text: line.modelData.sizeText; color: Theme.muted; font.pixelSize: 13 }
            MouseArea {
                anchors.fill: parent
                onClicked: mouse => {
                    const mode = (mouse.modifiers & Qt.ControlModifier) ? 1 : (mouse.modifiers & Qt.ShiftModifier) ? 2 : 0
                    p.activate("win.pane-pick", line.modelData.pos + ":" + mode)
                }
                onDoubleClicked: p.activate("win.pane-open", String(line.modelData.pos))
            }
        }
        // Empty space: activate the pane and clear its selection.
        MouseArea {
            z: -1
            anchors.fill: parent
            onClicked: p.activate("win.pane-pick", "-1:0")
        }
    }
}
