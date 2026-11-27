import QtQuick

// A toast that springs up from the bottom edge (DESIGN.md §3.5).
Rectangle {
    id: root
    property string text: ""
    property bool undo: false
    // What Undo runs: the operations journal's Undo, unless the toast is
    // about something else (a hidden sidebar entry passes its own action).
    property string undoAction: "win.undo"
    property string undoParam: ""
    function show(message, withUndo, action, param) {
        text = message
        undo = !!withUndo
        undoAction = action || "win.undo"
        undoParam = param || ""
        shown = true
        hide.restart()
    }
    property bool shown: false

    width: row.implicitWidth + 40
    height: 44
    radius: 22
    color: Theme.toastBg
    anchors.horizontalCenter: parent.horizontalCenter
    y: parent.height - (shown ? height + 28 : -10)
    opacity: shown ? 1 : 0
    visible: opacity > 0.01
    Behavior on y { enabled: Theme.motion; SpringAnimation { spring: 3.5; damping: 0.3 } }
    Behavior on opacity { NumberAnimation { duration: 160 } }
    layer.enabled: Theme.effects
    layer.samplerName: "tex"
    layer.effect: FxShadow { shadowOpacity: 0.3; shadowBlur: 0.8; shadowVerticalOffset: 6 }

    Accessible.role: Accessible.AlertMessage
    Accessible.name: text

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 16
        Text { id: label; anchors.verticalCenter: parent.verticalCenter; text: root.text; color: Theme.fillInk(1); font.pixelSize: 14 }
        Rectangle {
            visible: root.undo
            anchors.verticalCenter: parent.verticalCenter
            width: undoLabel.implicitWidth + 22; height: 28; radius: 14
            color: undoHover.hovered ? Theme.fillInk(0.22) : Theme.fillInk(0.12)
            Accessible.role: Accessible.Button
            Accessible.name: p.t("Undo")
            Accessible.onPressAction: { root.shown = false; p.activate(root.undoAction, root.undoParam) }
            Text { id: undoLabel; anchors.centerIn: parent; text: p.t("Undo"); color: Theme.fillInk(1); font.pixelSize: 13; font.weight: Font.Bold }
            HoverHandler { id: undoHover }
            TapHandler { onTapped: { root.shown = false; p.activate(root.undoAction, root.undoParam) } }
        }
    }
    Timer { id: hide; interval: root.undo ? 7000 : 4000; onTriggered: root.shown = false }
}
