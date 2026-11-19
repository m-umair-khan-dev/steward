import QtQuick
import QtQuick.Effects

// A toast that springs up from the bottom edge (DESIGN.md §3.5).
Rectangle {
    id: root
    property string text: ""
    function show(message) {
        text = message
        shown = true
        hide.restart()
    }
    property bool shown: false

    width: label.implicitWidth + 40
    height: 44
    radius: 22
    color: Theme.toastBg
    anchors.horizontalCenter: parent.horizontalCenter
    y: parent.height - (shown ? height + 28 : -10)
    opacity: shown ? 1 : 0
    visible: opacity > 0.01
    Behavior on y { enabled: Theme.motion; SpringAnimation { spring: 3.5; damping: 0.3 } }
    Behavior on opacity { NumberAnimation { duration: 160 } }
    layer.enabled: true
    layer.effect: MultiEffect { shadowEnabled: true; shadowOpacity: 0.3; shadowBlur: 0.8; shadowVerticalOffset: 6 }

    Accessible.role: Accessible.AlertMessage
    Accessible.name: text

    Text { id: label; anchors.centerIn: parent; text: root.text; color: "white"; font.pixelSize: 14 }
    Timer { id: hide; interval: 4000; onTriggered: root.shown = false }
}
