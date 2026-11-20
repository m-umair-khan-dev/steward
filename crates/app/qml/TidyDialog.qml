import QtQuick
import QtQuick.Controls.Basic

// The Tidy plan (P4-T03): what moves where, and what stays and why.
Sheet {
    id: root
    property var info: ({ groups: [], left: [] })
    title: p.t("Tidy “") + (info.folder || "") + "”"
    cardWidth: 560
    cardHeight: 600
    function close() { p.activate("win.tidy-cancel", "") }

    Item {
        width: parent.width
        height: root.height > 0 ? Math.min(root.height - 140, 520) : 480
        Text {
            id: summary
            width: parent.width
            wrapMode: Text.WordWrap
            text: root.info.summary || ""
            color: Theme.ink
            font.pixelSize: 14
        }
        Flickable {
            anchors.top: summary.bottom; anchors.topMargin: 12
            anchors.bottom: buttons.top; anchors.bottomMargin: 12
            width: parent.width
            contentHeight: lists.implicitHeight
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            Column {
                id: lists
                width: parent.width
                spacing: 10
                Text { visible: (root.info.groups || []).length > 0; text: p.t("WILL MOVE"); color: Theme.faint
                       font.pixelSize: 12; font.weight: Font.DemiBold; font.letterSpacing: 0.6 }
                Repeater {
                    model: root.info.groups || []
                    delegate: Column {
                        id: group
                        required property var modelData
                        width: lists.width
                        spacing: 3
                        Accessible.role: Accessible.Grouping
                        Accessible.name: p.t("Will move to ") + modelData.folder
                        Row {
                            spacing: 8
                            Rectangle { width: 12; height: 12; radius: 3; color: Theme.deep(group.modelData.kind)
                                        anchors.verticalCenter: parent.verticalCenter }
                            Text { text: group.modelData.folder + "  " + group.modelData.count; color: Theme.ink
                                   font.pixelSize: 14; font.weight: Font.DemiBold }
                            Text { visible: !!group.modelData["new"]; text: p.t("new folder"); color: Theme.accentText
                                   font.pixelSize: 12; anchors.verticalCenter: parent.verticalCenter }
                        }
                        Repeater {
                            model: group.modelData.files
                            delegate: Text { required property string modelData; x: 20; width: lists.width - 20
                                             text: modelData; elide: Text.ElideMiddle; color: Theme.muted; font.pixelSize: 13
                                             Accessible.role: Accessible.ListItem; Accessible.name: modelData }
                        }
                    }
                }
                Text { visible: (root.info.left || []).length > 0; text: p.t("LEFT WHERE THEY ARE"); color: Theme.faint
                       font.pixelSize: 12; font.weight: Font.DemiBold; font.letterSpacing: 0.6; topPadding: 6 }
                Repeater {
                    model: root.info.left || []
                    delegate: Text {
                        required property var modelData
                        width: lists.width
                        text: modelData.name + " — " + modelData.reason
                        elide: Text.ElideMiddle
                        color: Theme.muted; font.pixelSize: 13
                        Accessible.role: Accessible.ListItem
                        Accessible.name: modelData.name
                    }
                }
            }
        }
        Row {
            id: buttons
            anchors.bottom: parent.bottom
            anchors.right: parent.right
            spacing: 8
            PillButton { text: (root.info.empty ? p.t("Close") : p.t("Cancel")); onClicked: root.close() }
            PillButton { visible: !root.info.empty; text: p.t("Tidy"); primary: true; onClicked: p.activate("win.tidy-run", "") }
        }
    }
}
