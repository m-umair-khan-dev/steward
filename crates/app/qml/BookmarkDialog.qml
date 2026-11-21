import QtQuick
import QtQuick.Controls.Basic

// Edit a bookmark: its title, tags and collection (P7-T08).
Sheet {
    id: root
    property var info: ({ collections: [] })
    title: p.t("Edit Bookmark")
    cardWidth: 480
    onVisibleChanged: if (visible) { name.text = info.title || ""; tags.text = info.tags || ""
                                     collection.text = info.collection || ""; name.forceActiveFocus() }
    function save() {
        p.activate("win.bookmark-edit", info.id + "|" + name.text.replace(/\|/g, " ") + "|"
                   + tags.text.replace(/\|/g, " ") + "|" + collection.text.replace(/\|/g, " "))
    }

    component Label_: Text { color: Theme.muted; font.pixelSize: 12; font.weight: Font.DemiBold; topPadding: 4 }

    component Field: TextField {
        width: parent.width
        color: Theme.ink
        background: Rectangle { radius: 9; color: Theme.pane; border.color: parent.activeFocus ? Theme.accent : Theme.line }
        onAccepted: root.save()
    }

    Column {
        width: parent.width
        spacing: 10
        Text { width: parent.width; elide: Text.ElideMiddle; text: root.info.url || ""; color: Theme.muted; font.pixelSize: 12 }
        Label_ { text: p.t("Title") }
        Field { id: name; placeholderText: p.t("Title"); Accessible.name: p.t("Title") }
        Label_ { text: p.t("Tags") }
        Field { id: tags; placeholderText: p.t("Tags, separated by commas"); Accessible.name: p.t("Tags") }
        Label_ { text: p.t("Collection") }
        Field { id: collection; placeholderText: p.t("Collection (a new name makes one)"); Accessible.name: p.t("Collection") }
        Flow {
            width: parent.width; spacing: 6
            Repeater {
                model: root.info.collections || []
                delegate: Rectangle {
                    required property string modelData
                    height: 26; radius: 13; width: chip.implicitWidth + 18
                    color: collection.text === modelData ? Theme.alpha(Theme.accent, 0.18) : Theme.sunken
                    Accessible.role: Accessible.Button
                    Accessible.name: p.t("Collection ") + modelData
                    Accessible.onPressAction: collection.text = modelData
                    Text { id: chip; anchors.centerIn: parent; text: parent.modelData; color: Theme.ink; font.pixelSize: 12 }
                    TapHandler { onTapped: collection.text = parent.modelData }
                }
            }
        }
        Row {
            anchors.right: parent.right
            spacing: 8
            PillButton { text: p.t("Delete"); destructive: true; onClicked: p.activate("win.bookmark-delete", String(root.info.id)) }
            PillButton { text: p.t("Cancel"); onClicked: root.close() }
            PillButton { text: p.t("Save"); primary: true; onClicked: root.save() }
        }
    }
}
