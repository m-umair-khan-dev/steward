import QtQuick

// A round button with one of our icons. `checked` fills it with the accent.
Rectangle {
    id: root
    property string icon: ""
    property string label: ""
    property bool checked: false
    property bool flat: false
    property int iconSize: 18
    signal clicked()

    implicitWidth: 34
    implicitHeight: 34
    radius: height / 2
    color: checked ? Theme.accent
                   : (hover.hovered ? Theme.hover : (flat ? "transparent" : Theme.track))
    Behavior on color { ColorAnimation { duration: 120 } }
    opacity: enabled ? 1 : 0.4

    Accessible.role: Accessible.Button
    Accessible.name: label
    Accessible.onPressAction: root.clicked()

    Icon {
        anchors.centerIn: parent
        name: root.icon
        size: root.iconSize
        color: root.checked ? "white" : Theme.ink
    }
    HoverHandler { id: hover }
    TapHandler { onTapped: root.clicked() }
}
