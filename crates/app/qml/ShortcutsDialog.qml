import QtQuick
import QtQuick.Controls.Basic

// Keyboard shortcuts, from the same table that creates them (actions.rs).
Sheet {
    id: root
    property var accels: []
    title: p.t("Keyboard Shortcuts")
    cardWidth: 520
    cardHeight: 560
    ListView {
        anchors.fill: parent
        clip: true
        model: root.accels
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {}
        delegate: Item {
            required property var modelData
            width: ListView.view.width
            height: 34
            // Every key bound to the action, alternatives joined with "or".
            Accessible.role: Accessible.StaticText
            Accessible.name: modelData.description + ": " + modelData.keys.join(p.t(" or "))
            Text { anchors.verticalCenter: parent.verticalCenter; text: parent.modelData.description
                   color: Theme.ink; font.pixelSize: 13; width: parent.width - keys.width - 12; elide: Text.ElideRight }
            Row {
                id: keys
                anchors.right: parent.right; anchors.rightMargin: 8
                anchors.verticalCenter: parent.verticalCenter
                spacing: 4
                Repeater {
                    model: parent.parent.modelData.keys.slice(0, 1)[0].split(/\+(?!$)/).filter(k => k !== "")
                    delegate: Rectangle {
                        required property string modelData
                        width: Math.max(26, key.implicitWidth + 14); height: 24; radius: 6
                        color: Theme.track; border.color: Theme.line
                        Text { id: key; anchors.centerIn: parent; text: parent.modelData; color: Theme.ink; font.pixelSize: 12 }
                    }
                }
            }
        }
    }
}
