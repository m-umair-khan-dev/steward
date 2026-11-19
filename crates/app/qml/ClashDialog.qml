import QtQuick
import QtQuick.Controls.Basic

// A name already exists where an item is going (P2-T06).
Sheet {
    id: root
    property var info: ({ name: "", folder: "" })
    title: "Item Already Exists"
    cardWidth: 460
    function close() { p.answer("skip", false) }

    Column {
        width: parent.width
        spacing: 14
        Text {
            width: parent.width
            wrapMode: Text.WordWrap
            text: "“" + (root.info.name || "") + "” is already in “" + (root.info.folder || "") + "”."
            color: Theme.ink
            font.pixelSize: 14
        }
        Text {
            width: parent.width
            wrapMode: Text.WordWrap
            text: "Replace puts the existing item in the Trash, so Undo can bring it back."
            color: Theme.muted
            font.pixelSize: 13
        }
        CheckBox {
            id: all
            text: "Do this for every clash"
            Accessible.name: "Apply to all"
        }
        Row {
            anchors.right: parent.right
            spacing: 8
            PillButton { text: "Skip"; onClicked: p.answer("skip", all.checked) }
            PillButton { text: "Keep Both"; onClicked: p.answer("keep-both", all.checked) }
            PillButton { text: "Replace"; primary: true; onClicked: p.answer("replace", all.checked) }
        }
    }
}
