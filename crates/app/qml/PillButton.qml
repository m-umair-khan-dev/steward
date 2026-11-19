import QtQuick

// A text button: filled with the accent (primary), red (destructive) or quiet.
Rectangle {
    id: root
    property string text: ""
    property bool primary: false
    property bool destructive: false
    signal clicked()

    implicitWidth: label.implicitWidth + 32
    implicitHeight: 36
    radius: 18
    color: destructive ? (hover.hovered ? "#b02a2a" : "#c53434")
                       : primary ? (hover.hovered ? Qt.darker(Theme.accent, 1.1) : Theme.accent)
                                 : (hover.hovered ? Theme.hover : Theme.track)
    Behavior on color { ColorAnimation { duration: 120 } }
    opacity: enabled ? 1 : 0.45
    Accessible.role: Accessible.Button
    Accessible.name: text
    Accessible.onPressAction: root.clicked()

    Text {
        id: label
        anchors.centerIn: parent
        text: root.text
        color: root.primary || root.destructive ? "white" : Theme.ink
        font.pixelSize: 14
        font.weight: Font.DemiBold
    }
    HoverHandler { id: hover }
    TapHandler { onTapped: root.clicked() }
}
