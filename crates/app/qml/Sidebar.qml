import QtQuick
import QtQuick.Controls.Basic

// The sidebar: inset rounded panel with the app row, Favorites, Locations and
// the computer row (DESIGN.md §3.3; reference smart-downloads.png).
Rectangle {
    id: root
    property var model: ({ entries: [], selected: "", free: "", usage: 0 })
    property bool overlay: false
    signal collapse()

    radius: 16
    color: Theme.sidebar
    border.color: Theme.line

    readonly property var favorites: (model.entries || []).filter(e => e.section === "favorites")
    readonly property var locations: (model.entries || []).filter(e => e.section === "locations")

    // App row; also a handle to move the window.
    property bool locationsOpen: false

    Item {
        id: appRow
        x: 18; y: 18
        width: parent.width - 30
        height: 42
        DragHandler { target: null; onActiveChanged: if (active) root.Window.window.startSystemMove() }
        Rectangle {
            id: badge
            width: 34; height: 34; radius: 10
            anchors.verticalCenter: parent.verticalCenter
            gradient: Gradient {
                GradientStop { position: 0; color: "#a497f2" }
                GradientStop { position: 1; color: "#6a55db" }
            }
            Icon { anchors.centerIn: parent; name: "folder"; size: 19; color: "white" }
        }
        Text {
            anchors.left: badge.right; anchors.leftMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            text: "Cairn"
            color: Theme.ink
            font.pixelSize: 24; font.weight: Font.Bold
        }
        IconButton {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            width: 30; height: 30
            flat: true
            icon: "sidebar"
            iconSize: 17
            label: "Collapse sidebar"
            visible: !root.overlay
            onClicked: root.collapse()
        }
    }

    Flickable {
        anchors.top: appRow.bottom; anchors.topMargin: 10
        anchors.left: parent.left; anchors.right: parent.right
        anchors.bottom: computer.top
        contentHeight: column.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds

        Column {
            id: column
            width: parent.width
            spacing: 2

            Heading { text: "Favorites" }
            Item {
                width: parent.width
                height: favoritesList.implicitHeight
                Accessible.role: Accessible.List
                Accessible.name: "Sidebar"
                Column {
                    id: favoritesList
                    width: parent.width
                    spacing: 2
                    Repeater { model: root.favorites; delegate: Row_ {} }
                }
            }
            // "Locations ›" opens and closes, as in the reference.
            Heading {
                id: locationsHeading
                text: "Locations"
                visible: root.locations.length > 0
                Accessible.role: Accessible.Button
                Accessible.name: "Locations"
                Accessible.onPressAction: root.locationsOpen = !root.locationsOpen
                Icon {
                    x: parent.implicitWidth + 4
                    y: parent.topPadding + 2
                    name: "chevron"; size: 14; color: Theme.muted
                    rotation: root.locationsOpen ? 90 : 0
                    Behavior on rotation { enabled: Theme.motion; NumberAnimation { duration: 160 } }
                }
                TapHandler { onTapped: root.locationsOpen = !root.locationsOpen }
            }
            Item {
                width: parent.width
                height: root.locationsOpen ? locationsList.implicitHeight : 0
                clip: true
                visible: root.locations.length > 0 && height > 0
                Behavior on height { enabled: Theme.motion; NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
                Accessible.role: Accessible.List
                Accessible.name: "Locations"
                Column {
                    id: locationsList
                    width: parent.width
                    spacing: 2
                    Repeater { model: root.locations; delegate: Row_ {} }
                }
            }
        }
    }

    // The computer row: free space on the home drive.
    Rectangle {
        id: computer
        anchors.bottom: parent.bottom; anchors.bottomMargin: 10
        anchors.left: parent.left; anchors.leftMargin: 12
        anchors.right: parent.right; anchors.rightMargin: 12
        height: 58
        radius: 10
        color: computerHover.hovered ? Theme.hover : "transparent"
        Accessible.role: Accessible.Button
        Accessible.name: "Computer"
        Accessible.onPressAction: p.activate("win.go-to", "/")
        Rectangle { width: parent.width; height: 1; color: Theme.line }
        Icon { x: 6; anchors.verticalCenter: parent.verticalCenter; name: "computer"; size: 30; color: Theme.ink }
        Column {
            x: 46; anchors.verticalCenter: parent.verticalCenter
            width: parent.width - 52
            spacing: 5
            Text { text: "Computer"; color: Theme.ink; font.pixelSize: 16; font.weight: Font.Bold }
            Row {
                spacing: 8
                Rectangle {
                    width: 84; height: 5; radius: 2.5; color: Theme.track
                    anchors.verticalCenter: parent.verticalCenter
                    Rectangle { width: parent.width * (root.model.usage || 0); height: 5; radius: 2.5; color: Theme.accent }
                }
                Text { text: root.model.free || ""; color: Theme.muted; font.pixelSize: 11 }
            }
        }
        HoverHandler { id: computerHover }
        TapHandler { onTapped: p.activate("win.go-to", "/") }
    }

    component Heading: Text {
        x: 22
        topPadding: 16
        bottomPadding: 6
        color: Theme.muted
        font.pixelSize: 14
        font.weight: Font.DemiBold
    }

    component Row_: Rectangle {
        id: row
        required property var modelData
        readonly property bool selected: root.model.selected === modelData.key
        x: 12
        width: root.width - 24
        height: 38
        radius: 10
        color: selected ? Theme.alpha(Theme.accent, 0.16) : (rowHover.hovered ? Theme.hover : "transparent")
        Behavior on color { ColorAnimation { duration: 120 } }
        Accessible.role: Accessible.Button
        Accessible.name: modelData.label
        Accessible.onPressAction: p.activate("win.go-to", modelData.path)

        Icon { x: 10; anchors.verticalCenter: parent.verticalCenter; name: row.modelData.icon; size: 20; color: Theme.accentText }
        Text {
            x: 42; anchors.verticalCenter: parent.verticalCenter
            width: parent.width - 52 - (eject.visible ? 26 : 0) - (count.visible ? count.implicitWidth + 8 : 0)
            elide: Text.ElideRight
            text: row.modelData.label
            color: Theme.ink
            font.pixelSize: 15
            font.weight: row.selected ? Font.Bold : Font.Normal
        }
        Text {
            id: count
            visible: !!row.modelData.count && !eject.visible
            anchors.right: parent.right; anchors.rightMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            text: row.modelData.count || ""
            color: Theme.faint
            font.pixelSize: 14
        }
        IconButton {
            id: eject
            visible: !!row.modelData.eject
            anchors.right: parent.right; anchors.rightMargin: 4
            anchors.verticalCenter: parent.verticalCenter
            width: 26; height: 26; flat: true; icon: "eject"; iconSize: 14
            label: "Eject " + row.modelData.label
            onClicked: p.activate("win.sidebar-eject", row.modelData.key)
        }
        HoverHandler { id: rowHover }
        FolderDrop {
            id: rowDrop
            anchors.fill: parent
            target: row.modelData.uri
        }
        Rectangle {
            anchors.fill: parent; radius: parent.radius
            color: "transparent"; border.color: Theme.accent; border.width: 2
            visible: rowDrop.containsDrag
        }
        TapHandler {
            acceptedButtons: Qt.LeftButton
            onTapped: p.activate("win.go-to", row.modelData.path)
        }
        TapHandler {
            acceptedButtons: Qt.RightButton
            onTapped: (point) => {
                menu.key = row.modelData.key
                menu.popup()
            }
        }
    }

    Menu {
        id: menu
        property string key: ""
        MenuItem { text: "Move Up"; onTriggered: p.activate("win.sidebar-move-up", menu.key) }
        MenuItem { text: "Move Down"; onTriggered: p.activate("win.sidebar-move-down", menu.key) }
        MenuItem { text: "Hide"; onTriggered: p.activate("win.sidebar-hide", menu.key) }
    }
}
