import QtQuick
import QtQuick.Effects

// Pill-shaped segmented control whose selection slides (DESIGN.md §3.4).
// Options: [{ id, label?, icon?, a11y }].
Rectangle {
    id: root
    property var options: []
    property string current: ""
    property string name: ""
    signal picked(string id)

    readonly property int index: Math.max(0, options.findIndex(o => o.id === current))
    implicitWidth: row.implicitWidth + 6
    implicitHeight: 36
    radius: height / 2
    // White pill with a thin border; the choice gets a soft grey fill (reference).
    color: Theme.pane
    border.color: Theme.line
    border.width: 1

    Accessible.role: Accessible.Grouping
    Accessible.name: name

    Rectangle {
        id: indicator
        y: 3
        height: parent.height - 6
        radius: height / 2
        color: Theme.dark ? "#2c2a3d" : "#ececf2"
        property Item target: repeater.count > root.index ? repeater.itemAt(root.index) : null
        x: target ? row.x + target.x : 3
        width: target ? target.width : 0
        Behavior on x { enabled: Theme.motion; SpringAnimation { spring: 5; damping: 0.4 } }
        Behavior on width { enabled: Theme.motion; SpringAnimation { spring: 5; damping: 0.4 } }
    }

    Row {
        id: row
        x: 3
        anchors.verticalCenter: parent.verticalCenter
        Repeater {
            id: repeater
            model: root.options
            delegate: Item {
                required property var modelData
                required property int index
                width: modelData.icon ? 36 : label.implicitWidth + 26
                height: 30
                Accessible.role: Accessible.Button
                Accessible.name: modelData.a11y || modelData.label || modelData.id
                Accessible.checked: root.current === modelData.id
                Accessible.onPressAction: root.picked(modelData.id)
                Text {
                    id: label
                    anchors.centerIn: parent
                    visible: !parent.modelData.icon
                    text: parent.modelData.label || ""
                    color: Theme.ink
                    font.pixelSize: 13
                    font.weight: root.current === parent.modelData.id ? Font.DemiBold : Font.Normal
                }
                Icon {
                    anchors.centerIn: parent
                    visible: !!parent.modelData.icon
                    name: parent.modelData.icon || ""
                    size: 17
                }
                TapHandler { onTapped: root.picked(parent.modelData.id) }
            }
        }
    }
}
