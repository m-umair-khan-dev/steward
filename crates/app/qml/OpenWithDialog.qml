import QtQuick
import QtQuick.Controls.Basic

// Open With: the installed apps for the selected file's type.
Sheet {
    id: root
    property var info: ({ file: "", apps: [] })
    title: "Open With"
    cardWidth: 420
    Column {
        width: parent.width
        spacing: 6
        Text { width: parent.width; text: root.info.file || ""; color: Theme.muted; font.pixelSize: 13
               elide: Text.ElideMiddle; bottomPadding: 6 }
        Repeater {
            model: root.info.apps || []
            delegate: Rectangle {
                required property string modelData
                required property int index
                width: parent.width; height: 40; radius: 10
                color: hover.hovered ? Theme.hover : Theme.track
                Accessible.role: Accessible.Button
                Accessible.name: modelData
                Accessible.onPressAction: p.launch(index)
                Text { x: 14; anchors.verticalCenter: parent.verticalCenter; text: parent.modelData
                       color: Theme.ink; font.pixelSize: 14 }
                HoverHandler { id: hover }
                TapHandler { onTapped: p.launch(parent.index) }
            }
        }
    }
}
