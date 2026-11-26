import QtQuick

// A text button (rounded rectangle, as in the reference): accent-filled
// (primary), red (destructive) or white with a border.
Rectangle {
    id: root
    property string text: ""
    property bool primary: false
    property bool destructive: false
    signal clicked()

    implicitWidth: label.implicitWidth + 32
    implicitHeight: 36
    radius: 10
    color: destructive ? (hover.hovered ? Theme.dangerHover : Theme.danger)
                       : primary ? (hover.hovered ? Theme.accentFillHover : Theme.accentFill)
                                 : (hover.hovered ? Theme.sunken : Theme.pane)
    border.color: primary || destructive ? "transparent" : Theme.line
    border.width: 1
    Behavior on color { ColorAnimation { duration: 120 } }
    opacity: enabled ? 1 : 0.45
    Accessible.role: Accessible.Button
    Accessible.name: text
    Accessible.onPressAction: root.clicked()

    Text {
        id: label
        anchors.centerIn: parent
        text: root.text
        color: root.primary || root.destructive ? Theme.onAccent : Theme.ink
        font.pixelSize: 14
        font.weight: Font.DemiBold
    }
    HoverHandler { id: hover }
    TapHandler { onTapped: root.clicked() }
}
