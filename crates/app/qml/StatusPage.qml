import QtQuick
import QtQuick.Controls.Basic

// Empty, no-match, denied, gone and loading states (DESIGN.md §3.3).
Item {
    id: root
    property string icon: "folder"
    property string title: ""
    property string text: ""
    property bool loading: false

    Accessible.role: Accessible.StaticText
    Accessible.name: title

    Column {
        anchors.centerIn: parent
        spacing: 12
        width: Math.min(parent.width - 48, 420)
        BusyIndicator {
            visible: root.loading
            running: root.loading
            anchors.horizontalCenter: parent.horizontalCenter
        }
        Rectangle {
            visible: !root.loading
            anchors.horizontalCenter: parent.horizontalCenter
            width: 76; height: 76; radius: 22
            color: Theme.alpha(Theme.accent, 0.12)
            Icon { anchors.centerIn: parent; name: root.icon; size: 36; color: Theme.accentText }
        }
        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: root.title
            color: Theme.ink
            font.pixelSize: 20; font.weight: Font.Bold
            wrapMode: Text.WordWrap
        }
        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: root.text
            color: Theme.muted
            font.pixelSize: 14
            wrapMode: Text.WordWrap
        }
    }
}
