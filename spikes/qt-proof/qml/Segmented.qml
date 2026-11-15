import QtQuick
import QtQuick.Effects

// A pill-shaped segmented control whose selection slides between options.
Rectangle {
    id: root

    property var options: []
    property int current: 0
    signal picked(int index)

    width: row.implicitWidth + 6
    height: 36
    radius: height / 2
    color: Qt.rgba(0.09, 0.07, 0.16, 0.05)

    Accessible.role: Accessible.PageTabList

    Rectangle {
        id: indicator
        y: 3
        height: parent.height - 6
        radius: height / 2
        color: "white"
        x: row.x + (repeater.itemAt(root.current) ? repeater.itemAt(root.current).x : 0)
        width: repeater.itemAt(root.current) ? repeater.itemAt(root.current).width : 0
        Behavior on x { SpringAnimation { spring: 5; damping: 0.38 } }
        Behavior on width { SpringAnimation { spring: 5; damping: 0.38 } }
        layer.enabled: true
        layer.effect: MultiEffect { shadowEnabled: true; shadowOpacity: 0.18; shadowBlur: 0.4; shadowVerticalOffset: 1 }
    }

    Row {
        id: row
        x: 3
        anchors.verticalCenter: parent.verticalCenter
        Repeater {
            id: repeater
            model: root.options
            delegate: Item {
                required property int index
                required property string modelData
                width: label.implicitWidth + 28
                height: 30
                Accessible.role: Accessible.PageTab
                Accessible.name: modelData
                Text {
                    id: label
                    anchors.centerIn: parent
                    text: parent.modelData
                    color: "#16132a"
                    font.pixelSize: 13
                    font.weight: root.current === parent.index ? Font.DemiBold : Font.Normal
                }
                TapHandler { onTapped: root.picked(parent.index) }
            }
        }
    }
}
