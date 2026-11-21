import QtQuick
import QtQuick.Controls.Basic

// Add Link (Ctrl+Shift+L; P7-T08).
Sheet {
    id: root
    property var info: ({})
    title: p.t("Add Link")
    cardWidth: 460
    onVisibleChanged: if (visible) { address.text = ""; address.forceActiveFocus() }
    function save() { if (address.text.trim() !== "") p.activate("win.bookmark-add", address.text) }

    Column {
        width: parent.width
        spacing: 12
        TextField {
            id: address
            width: parent.width
            placeholderText: "https://"
            Accessible.name: p.t("Link address")
            color: Theme.ink
            background: Rectangle { radius: 9; color: Theme.pane; border.color: address.activeFocus ? Theme.accent : Theme.line }
            onAccepted: root.save()
        }
        Row {
            anchors.right: parent.right
            spacing: 8
            PillButton { text: p.t("Cancel"); onClicked: root.close() }
            PillButton { text: p.t("Add"); primary: true; onClicked: root.save() }
        }
    }
}
