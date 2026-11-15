import QtQuick
import QtQuick.Effects

// A tinted "glass" folder: coloured back with a tab, two sheets of paper, and a
// frosted front that really blurs what is behind it (ADR 0008 look).
Item {
    id: root

    property color deep: "#6a55db"
    property color light: "#a497f2"
    property string label: ""
    property string count: ""
    property string size: ""
    property bool forcedHover: false
    readonly property bool lifted: hover.hovered || forcedHover

    width: 148
    height: 150

    Accessible.role: Accessible.Button
    Accessible.name: label

    function alpha(c, a) { return Qt.rgba(c.r, c.g, c.b, a) }

    HoverHandler { id: hover; cursorShape: Qt.PointingHandCursor }

    Item {
        id: shape
        width: 132
        height: 106
        anchors.horizontalCenter: parent.horizontalCenter
        y: root.lifted ? 0 : 4
        scale: root.lifted ? 1.05 : 1.0
        Behavior on y { SpringAnimation { spring: 4; damping: 0.32 } }
        Behavior on scale { SpringAnimation { spring: 4; damping: 0.32 } }

        layer.enabled: true
        layer.effect: MultiEffect {
            shadowEnabled: true
            shadowColor: root.alpha(root.deep, 0.55)
            shadowBlur: root.lifted ? 0.9 : 0.5
            shadowVerticalOffset: root.lifted ? 8 : 3
            shadowOpacity: root.lifted ? 0.55 : 0.3
            Behavior on shadowBlur { NumberAnimation { duration: 180 } }
            Behavior on shadowVerticalOffset { NumberAnimation { duration: 180 } }
        }

        // Everything behind the glass front.
        Item {
            id: behind
            anchors.fill: parent
            Rectangle { x: 0; y: 0; width: 54; height: 26; radius: 10; color: root.deep }
            Rectangle { y: 12; width: parent.width; height: parent.height - 12; radius: 14; color: root.deep }
            Rectangle { x: 18; y: 18; width: 96; height: 52; radius: 6; color: "white"; rotation: -4; antialiasing: true }
            Rectangle { x: 20; y: 22; width: 96; height: 52; radius: 6; color: "#efedf8"; rotation: 3; antialiasing: true }
        }

        // Frosted front: a blurred copy of what is behind, masked to the front.
        ShaderEffectSource {
            id: behindTexture
            sourceItem: behind
            sourceRect: Qt.rect(front.x, front.y, front.width, front.height)
            visible: false
        }
        Rectangle {
            id: frontMask
            width: front.width; height: front.height; radius: front.radius
            visible: false
            layer.enabled: true
        }
        MultiEffect {
            x: front.x; y: front.y; width: front.width; height: front.height
            source: behindTexture
            blurEnabled: true
            blur: 1.0
            blurMax: 40
            maskEnabled: true
            maskSource: frontMask
            maskThresholdMin: 0.5
            maskSpreadAtMin: 1.0
        }
        Rectangle {
            id: front
            y: parent.height * 0.36
            width: parent.width
            height: parent.height - y
            radius: 12
            gradient: Gradient {
                GradientStop { position: 0.0; color: root.alpha(root.light, 0.70) }
                GradientStop { position: 1.0; color: root.alpha(root.deep, 0.78) }
            }
            border.color: Qt.rgba(1, 1, 1, 0.45)
            border.width: 1

            Text {
                x: 10; y: 7
                text: root.count
                color: "white"; font.pixelSize: 12; font.weight: Font.Bold
            }
            // A simple document symbol.
            Rectangle {
                anchors.centerIn: parent
                width: 16; height: 20; radius: 3
                color: "transparent"; border.color: Qt.rgba(1, 1, 1, 0.9); border.width: 2
                Rectangle { x: 4; y: 6; width: 8; height: 2; color: Qt.rgba(1, 1, 1, 0.9) }
                Rectangle { x: 4; y: 10; width: 8; height: 2; color: Qt.rgba(1, 1, 1, 0.9) }
            }
            Text {
                x: 10; anchors.bottom: parent.bottom; anchors.bottomMargin: 6
                text: root.size
                color: Qt.rgba(1, 1, 1, 0.92); font.pixelSize: 10
            }
        }
    }

    Text {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: shape.bottom
        anchors.topMargin: 10
        text: root.label
        color: "#16132a"
        font.pixelSize: 14
        elide: Text.ElideMiddle
        width: parent.width
        horizontalAlignment: Text.AlignHCenter
    }
}
