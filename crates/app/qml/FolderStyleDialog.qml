import QtQuick

// Symbol and colour for one folder (P4-T10).
Sheet {
    id: root
    property var info: ({ symbols: [], colors: [] })
    property string symbol: info.symbol || "document"
    property string color: info.color || "folders"
    title: "Symbol and Colour"
    cardWidth: 460
    onVisibleChanged: if (visible) { root.symbol = info.symbol || "document"; root.color = info.color || "folders" }

    Column {
        width: parent.width
        spacing: 14
        Row {
            spacing: 16
            GlassFolder { width: 120; kind: root.color; symbol: root.symbol; label: root.info.name || ""; enabled: false }
            Text { anchors.verticalCenter: parent.verticalCenter; width: 260; wrapMode: Text.WordWrap
                   text: "Choose how “" + (root.info.name || "") + "” looks. It keeps this look when renamed or moved."
                   color: Theme.muted; font.pixelSize: 13 }
        }
        Flow {
            width: parent.width
            spacing: 6
            Repeater {
                model: root.info.symbols || []
                delegate: Rectangle {
                    required property string modelData
                    width: 40; height: 40; radius: 10
                    color: root.symbol === modelData ? Theme.alpha(Theme.accent, 0.18) : Theme.track
                    border.color: root.symbol === modelData ? Theme.accent : "transparent"
                    Accessible.role: Accessible.Button
                    Accessible.name: "Symbol " + modelData
                    Accessible.onPressAction: root.symbol = modelData
                    Icon { anchors.centerIn: parent; name: parent.modelData; size: 20; color: Theme.ink }
                    MouseArea { anchors.fill: parent; onClicked: root.symbol = parent.modelData }
                }
            }
        }
        Row {
            spacing: 8
            Repeater {
                model: root.info.colors || []
                delegate: Rectangle {
                    required property string modelData
                    width: 28; height: 28; radius: 14
                    color: Theme.deep(modelData)
                    border.color: root.color === modelData ? Theme.ink : "transparent"
                    border.width: 2
                    Accessible.role: Accessible.Button
                    Accessible.name: "Colour " + modelData
                    Accessible.onPressAction: root.color = modelData
                    MouseArea { anchors.fill: parent; onClicked: root.color = parent.modelData }
                }
            }
        }
        Row {
            anchors.right: parent.right
            spacing: 8
            PillButton { text: "Automatic"; onClicked: p.activate("win.folder-style", "auto") }
            PillButton { text: "Cancel"; onClicked: root.close() }
            PillButton { text: "Apply"; primary: true; onClicked: p.activate("win.folder-style", root.symbol + "|" + root.color) }
        }
    }
}
