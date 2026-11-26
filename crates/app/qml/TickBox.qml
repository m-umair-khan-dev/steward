import QtQuick

// A tick box (Smart Clean rows): accent-filled with a white tick when on.
Rectangle {
    id: root
    property bool checked: false
    property string label: ""
    signal toggled()
    width: 22; height: 22; radius: 6
    color: checked ? Theme.accentFill : Theme.pane
    border.color: checked ? Theme.accentFill : Theme.faint
    border.width: 1.5
    opacity: enabled ? 1 : 0.4
    Accessible.role: Accessible.CheckBox
    Accessible.name: label
    Accessible.checkable: true
    Accessible.checked: checked
    Accessible.onPressAction: root.toggled()
    Accessible.onToggleAction: root.toggled()
    Icon { anchors.centerIn: parent; name: "check"; size: 14; color: Theme.onAccent; visible: root.checked }
    TapHandler { enabled: root.enabled; onTapped: root.toggled() }
}
