import QtQuick
import QtQuick.Controls.Basic

// A round button with one of our icons. `checked` fills it with the accent.
Rectangle {
    id: root
    property string icon: ""
    property string label: ""
    property bool checked: false
    property bool flat: false
    property int iconSize: 18
    // A tooltip on hover, for buttons whose icon alone is not clear.
    property string tip: ""
    signal clicked()

    implicitWidth: 34
    implicitHeight: 34
    radius: height / 2
    color: checked ? Theme.accentFill
                   : (hover.hovered ? (flat ? Theme.hover : Theme.sunken) : (flat ? "transparent" : Theme.pane))
    border.color: checked || flat ? "transparent" : Theme.line
    border.width: 1
    Behavior on color { ColorAnimation { duration: 120 } }
    opacity: enabled ? 1 : 0.4

    Accessible.role: Accessible.Button
    Accessible.name: label
    Accessible.onPressAction: root.clicked()

    Icon {
        anchors.centerIn: parent
        name: root.icon
        size: root.iconSize
        color: root.checked ? Theme.onAccent : Theme.ink
    }
    HoverHandler { id: hover }
    TapHandler { onTapped: root.clicked() }
    ToolTip.visible: tip !== "" && hover.hovered
    ToolTip.delay: 500
    ToolTip.text: tip
}
