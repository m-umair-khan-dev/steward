import QtQuick

// About Cairn.
Sheet {
    id: root
    property string version: ""
    title: p.t("About")
    cardWidth: 380
    Column {
        width: parent.width
        spacing: 8
        Rectangle {
            anchors.horizontalCenter: parent.horizontalCenter
            width: 72; height: 72; radius: 20
            gradient: Gradient {
                GradientStop { position: 0; color: "#a497f2" }
                GradientStop { position: 1; color: "#6a55db" }
            }
            Icon { anchors.centerIn: parent; name: "folder"; size: 38; color: "white" }
        }
        Text { anchors.horizontalCenter: parent.horizontalCenter; text: p.t("Cairn"); color: Theme.ink
               font.pixelSize: 22; font.weight: Font.Bold }
        Text { anchors.horizontalCenter: parent.horizontalCenter; text: p.t("Version ") + root.version; color: Theme.muted
               font.pixelSize: 13 }
        Text { width: parent.width; horizontalAlignment: Text.AlignHCenter; wrapMode: Text.WordWrap
               text: p.t("A file manager for Ubuntu that keeps folders tidy."); color: Theme.ink; font.pixelSize: 14
               topPadding: 6 }
        Text { anchors.horizontalCenter: parent.horizontalCenter; text: p.t("GPL-3.0-or-later"); color: Theme.faint
               font.pixelSize: 12; bottomPadding: 6 }
    }
}
