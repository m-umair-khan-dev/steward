import QtQuick
import QtQuick.Controls.Basic

// A name already exists where an item is going (P2-T06).
Sheet {
    id: root
    property var info: ({ name: "", folder: "" })
    title: p.t("Item Already Exists")
    cardWidth: 460
    function close() { p.answer("skip", false) }

    Column {
        width: parent.width
        spacing: 14
        Text {
            width: parent.width
            wrapMode: Text.WordWrap
            text: "“" + (root.info.name || "") + p.t("” is already in “") + (root.info.folder || "") + "”."
            color: Theme.ink
            font.pixelSize: 14
        }
        Text {
            width: parent.width
            wrapMode: Text.WordWrap
            text: p.t("Replace puts the existing item in the Trash, so Undo can bring it back.")
            color: Theme.muted
            font.pixelSize: 13
        }
        CheckBox {
            id: all
            text: p.t("Do this for every clash")
            Accessible.name: p.t("Apply to all")
            // Purple, rounded (reference: smart-clean.png).
            indicator: Rectangle {
                x: all.leftPadding
                anchors.verticalCenter: parent.verticalCenter
                width: 20; height: 20; radius: 6
                color: all.checked ? Theme.accentFill : Theme.pane
                border.color: all.checked ? Theme.accentFill : Theme.faint
                border.width: 1.5
                Icon { anchors.centerIn: parent; visible: all.checked; name: "check"; size: 14; color: Theme.accentInk }
            }
            contentItem: Text {
                leftPadding: all.indicator.width + 8
                text: all.text
                color: Theme.ink
                font.pixelSize: 13
                verticalAlignment: Text.AlignVCenter
            }
        }
        Row {
            anchors.right: parent.right
            spacing: 8
            PillButton { text: p.t("Skip"); onClicked: p.answer("skip", all.checked) }
            PillButton { text: p.t("Keep Both"); onClicked: p.answer("keep-both", all.checked) }
            PillButton { text: p.t("Replace"); primary: true; onClicked: p.answer("replace", all.checked) }
        }
    }
}
