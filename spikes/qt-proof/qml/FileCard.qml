import QtQuick
import QtQuick.Effects

// A file card: preview area, coloured type badge, name, kind and size.
Item {
    id: root

    property string name: ""
    property string meta: ""
    property string badge: ""
    property color tint: "#2c6cd6"
    property bool forcedHover: false
    readonly property bool lifted: hover.hovered || forcedHover

    width: 196
    height: 156

    Accessible.role: Accessible.Button
    Accessible.name: name

    HoverHandler { id: hover; cursorShape: Qt.PointingHandCursor }

    Rectangle {
        id: card
        width: parent.width
        height: parent.height
        y: root.lifted ? -4 : 0
        Behavior on y { SpringAnimation { spring: 4; damping: 0.3 } }
        radius: 14
        color: "white"
        border.color: root.lifted ? Qt.rgba(0.42, 0.34, 0.86, 0.55) : Qt.rgba(0.09, 0.07, 0.16, 0.08)
        border.width: 1
        Behavior on border.color { ColorAnimation { duration: 160 } }
        clip: true

        layer.enabled: true
        layer.effect: MultiEffect {
            shadowEnabled: true
            shadowColor: "#1a1240"
            shadowOpacity: root.lifted ? 0.22 : 0.06
            shadowBlur: root.lifted ? 0.8 : 0.3
            shadowVerticalOffset: root.lifted ? 8 : 1
            Behavior on shadowOpacity { NumberAnimation { duration: 180 } }
        }

        Rectangle {
            id: preview
            width: parent.width
            height: 100
            color: "#f1f0f6"
            Rectangle {
                anchors.centerIn: parent
                width: 120; height: 70; radius: 8
                gradient: Gradient {
                    orientation: Gradient.Horizontal
                    GradientStop { position: 0.0; color: Qt.lighter(root.tint, 1.5) }
                    GradientStop { position: 1.0; color: root.tint }
                }
                opacity: 0.85
            }
        }
        Rectangle {
            id: badgeBox
            x: 10; anchors.verticalCenter: info.verticalCenter
            width: badgeText.implicitWidth + 8; height: 15; radius: 4
            color: root.tint
            Text { id: badgeText; anchors.centerIn: parent; text: root.badge; color: "white"; font.pixelSize: 8; font.weight: Font.Black }
        }
        Column {
            id: info
            anchors.left: badgeBox.right; anchors.leftMargin: 8
            anchors.right: parent.right; anchors.rightMargin: 10
            y: preview.height + 10
            Text { width: parent.width; text: root.name; elide: Text.ElideMiddle; color: "#16132a"; font.pixelSize: 13 }
            Text { width: parent.width; text: root.meta; elide: Text.ElideRight; color: "#5d5a72"; font.pixelSize: 11 }
        }
    }
}
