import QtQuick
import QtQuick.Controls.Basic

// New bookmark collection (P7-T08).
Sheet {
    id: root
    property var info: ({})
    title: p.t("New Collection")
    cardWidth: 420
    onVisibleChanged: if (visible) { name.text = ""; name.forceActiveFocus() }
    function save() { if (name.text.trim() !== "") p.activate("win.collection-create", name.text) }

    Column {
        width: parent.width
        spacing: 12
        TextField {
            id: name
            width: parent.width
            placeholderText: p.t("Name, e.g. Reading")
            Accessible.name: p.t("Collection name")
            color: Theme.ink
            background: Rectangle { radius: 9; color: Theme.pane; border.color: name.activeFocus ? Theme.accent : Theme.line }
            onAccepted: root.save()
        }
        Row {
            anchors.right: parent.right
            spacing: 8
            PillButton { text: p.t("Cancel"); onClicked: root.close() }
            PillButton { text: p.t("Create"); primary: true; onClicked: root.save() }
        }
    }
}
