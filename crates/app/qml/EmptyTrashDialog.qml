import QtQuick

// Empty Trash always asks (DESIGN.md §1, rule 2).
Sheet {
    id: root
    property var info: ({ count: "" })
    title: "Empty the Trash?"
    cardWidth: 420
    Column {
        width: parent.width
        spacing: 16
        Text {
            width: parent.width
            wrapMode: Text.WordWrap
            text: (root.info.count || "") + " will be deleted for good. This cannot be undone."
            color: Theme.ink
            font.pixelSize: 14
        }
        Row {
            anchors.right: parent.right
            spacing: 8
            PillButton { text: "Cancel"; onClicked: root.close() }
            PillButton { text: "Empty Trash"; destructive: true; onClicked: p.activate("win.empty-trash-confirm", "") }
        }
    }
}
