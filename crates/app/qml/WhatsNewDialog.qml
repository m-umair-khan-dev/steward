import QtQuick

// "What's New" (P6-T02): once after an update, from this version's release
// notes (the same file as the website's changelog).
Sheet {
    id: root
    property var info: ({ items: [] })
    title: "What’s New in Cairn " + (info.version || "")
    cardWidth: 520
    Column {
        width: parent.width
        spacing: 10
        Text { text: root.info.title || ""; color: Theme.ink; font.pixelSize: 20; font.weight: Font.Bold
               Accessible.role: Accessible.Heading; Accessible.name: text }
        Repeater {
            model: root.info.items || []
            delegate: Row {
                required property string modelData
                spacing: 10
                width: parent.width
                Rectangle { width: 6; height: 6; radius: 3; color: Theme.accent; y: 8 }
                Text {
                    width: parent.width - 16
                    wrapMode: Text.WordWrap
                    textFormat: Text.StyledText
                    text: parent.modelData
                    color: Theme.ink; font.pixelSize: 14
                    Accessible.role: Accessible.StaticText
                    Accessible.name: text.replace(/<[^>]*>/g, "")
                }
            }
        }
        Item {
            width: parent.width; height: 44
            PillButton { anchors.right: parent.right; anchors.bottom: parent.bottom; primary: true; text: "Continue"
                         onClicked: p.activate("win.welcome-done", "") }
        }
    }
}
