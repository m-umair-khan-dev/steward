import QtQuick
import QtQuick.Controls.Basic

// A dialog drawn inside the window (scrim plus a rounded card). It closes
// through the presenter so the state the tests read stays true.
FocusScope {
    id: root
    property string title: ""
    property int cardWidth: 460
    property int cardHeight: 0
    default property alias content: body.data
    readonly property bool open: visible

    anchors.fill: parent
    z: 100
    visible: false
    onVisibleChanged: if (visible) card.forceActiveFocus()
    function close() { p.shown("") }
    // Gives the keyboard back to the card (Escape closes from there).
    function refocus() { card.forceActiveFocus() }

    Rectangle {
        anchors.fill: parent
        color: Theme.scrim(Theme.dark ? 0.55 : 0.32)
        opacity: root.visible ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 150 } }
        // A TapHandler does not stop a tap from reaching handlers underneath,
        // so this one also sees taps on the card (a Preferences tab used to
        // close the dialog): only a tap outside the card closes it.
        TapHandler {
            onTapped: eventPoint => {
                const at = card.mapFromItem(parent, eventPoint.position.x, eventPoint.position.y)
                if (!card.contains(at)) root.close()
            }
        }
    }

    Rectangle {
        id: card
        anchors.centerIn: parent
        width: Math.min(root.cardWidth, root.width - 48)
        height: root.cardHeight > 0 ? Math.min(root.cardHeight, root.height - 48) : Math.min(body.implicitHeight + 76, root.height - 48)
        radius: 18
        color: Theme.card
        border.color: Theme.line
        focus: true
        scale: root.visible ? 1 : 0.94
        Behavior on scale { enabled: Theme.motion; SpringAnimation { spring: 4; damping: 0.35 } }
        layer.enabled: Theme.effects
        layer.samplerName: "tex"
        layer.effect: FxShadow { shadowOpacity: 0.3; shadowBlur: 1.0; shadowVerticalOffset: 10 }
        Keys.onEscapePressed: root.close()
        TapHandler {} // keeps clicks on the card from reaching the scrim

        Accessible.role: Accessible.Dialog
        Accessible.name: root.title

        Text {
            x: 22; y: 20
            text: root.title
            color: Theme.ink
            font.pixelSize: 17; font.weight: Font.Bold
        }
        IconButton {
            anchors.right: parent.right; anchors.rightMargin: 14
            y: 14
            width: 30; height: 30; iconSize: 14
            icon: "close"; label: p.t("Close")
            onClicked: root.close()
        }
        Item {
            id: body
            x: 22; y: 58
            width: parent.width - 44
            height: parent.height - 76
            implicitHeight: childrenRect.height
            clip: true
        }
    }
}
