import QtQuick

// The icon rail shown while the sidebar is collapsed (DESIGN.md §3.3).
// Pointing at it slides the full sidebar over the content (Main.qml).
Rectangle {
    id: root
    property var model: ({ entries: [], selected: "" })
    signal expand()
    signal hoveredChanged_(bool hovered)

    width: 58
    radius: 16
    color: Theme.sidebar
    border.color: Theme.line
    Accessible.role: Accessible.Grouping
    Accessible.name: "Sidebar rail"

    HoverHandler { onHoveredChanged: root.hoveredChanged_(hovered) }

    Column {
        anchors.horizontalCenter: parent.horizontalCenter
        y: 14
        spacing: 6
        Rectangle {
            width: 34; height: 34; radius: 10
            anchors.horizontalCenter: parent.horizontalCenter
            gradient: Gradient {
                GradientStop { position: 0; color: "#a497f2" }
                GradientStop { position: 1; color: "#6a55db" }
            }
            Icon { anchors.centerIn: parent; name: "folder"; size: 17; color: "white" }
            Accessible.role: Accessible.Button
            Accessible.name: "Expand sidebar"
            Accessible.onPressAction: root.expand()
            TapHandler { onTapped: root.expand() }
        }
        Item { width: 1; height: 10 }
        Repeater {
            model: root.model.entries || []
            delegate: IconButton {
                required property var modelData
                anchors.horizontalCenter: parent.horizontalCenter
                width: 40; height: 40; radius: 10
                flat: true
                icon: modelData.icon
                label: modelData.label
                color: root.model.selected === modelData.key ? Theme.alpha(Theme.accent, 0.16)
                                                               : (hovered ? Theme.hover : "transparent")
                property bool hovered: false
                onClicked: p.activate("win.go-to", modelData.path)
            }
        }
    }
    IconButton {
        anchors.bottom: parent.bottom; anchors.bottomMargin: 12
        anchors.horizontalCenter: parent.horizontalCenter
        flat: true; icon: "computer"; label: "Computer"
        onClicked: p.activate("win.go-to", "/")
    }
}
