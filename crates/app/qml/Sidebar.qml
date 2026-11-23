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
        Image {
            id: badge
            width: 34; height: 34
            anchors.verticalCenter: parent.verticalCenter
            source: "qrc:/brand/app.svg"
            sourceSize: Qt.size(width * 2, height * 2)
            smooth: true
        }
        Text {
            anchors.left: badge.right; anchors.leftMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            text: p.t("Cairn")
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
            label: p.t("Collapse sidebar")
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

            // Workspaces (P4-T08), with "+" to make one.
            Item {
                width: parent.width; height: wsHeading.implicitHeight
                Heading { id: wsHeading; text: p.t("Workspaces") }
                IconButton {
                    anchors.right: parent.right; anchors.rightMargin: 14
                    y: wsHeading.topPadding - 4
                    width: 24; height: 24; iconSize: 13; flat: true; icon: "plus"; label: p.t("New workspace")
                    onClicked: p.activate("win.workspace-new", "")
                }
            }
            Item {
                width: parent.width
                height: wsList.implicitHeight
                Accessible.role: Accessible.List
                Accessible.name: p.t("Workspaces")
                Column {
                    id: wsList
                    width: parent.width
                    spacing: 2
                    Repeater { model: root.model.workspaces || []; delegate: WorkspaceRow {} }
                }
            }
            // Bookmarks (P7-T08): all, favourites, then collections; "+" makes one.
            Item {
                width: parent.width; height: bmHeading.implicitHeight
                Heading { id: bmHeading; text: p.t("Bookmarks") }
                IconButton {
                    anchors.right: parent.right; anchors.rightMargin: 14
                    y: bmHeading.topPadding - 4
                    width: 24; height: 24; iconSize: 13; flat: true; icon: "plus"; label: p.t("New collection")
                    onClicked: p.activate("win.collection-new", "")
                }
            }
            Item {
                width: parent.width
                height: bmList.implicitHeight
                Accessible.role: Accessible.List
                Accessible.name: p.t("Bookmarks")
                Column {
                    id: bmList
                    width: parent.width
                    spacing: 2
                    Repeater { model: root.model.bookmarks || []; delegate: ToolRow {} }
                }
            }
            Heading { text: p.t("Favourites") }
            Item {
                width: parent.width
                height: favoritesList.implicitHeight
                Accessible.role: Accessible.List
                Accessible.name: p.t("Sidebar")
                Column {
                    id: favoritesList
                    width: parent.width
                    spacing: 2
                    Repeater { model: root.favorites; delegate: Row_ {} }
                }
            }
            // Tools (Phase 5): Free Up Space and Installed.
            Heading { text: p.t("Tools") }
            Item {
                width: parent.width
                height: toolsList.implicitHeight
                Accessible.role: Accessible.List
                Accessible.name: p.t("Tools")
                Column {
                    id: toolsList
                    width: parent.width
                    spacing: 2
                    Repeater { model: root.model.tools || []; delegate: ToolRow {} }
                }
            }
            // "Locations ›" opens and closes, as in the reference.
            Heading {
                id: locationsHeading
                text: p.t("Locations")
                visible: root.locations.length > 0
                Accessible.role: Accessible.Button
                Accessible.name: p.t("Locations")
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
                Accessible.name: p.t("Locations")
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
        color: root.model.page === "about" ? Theme.alpha(Theme.accent, 0.16) : (computerHover.hovered ? Theme.hover : "transparent")
        Accessible.role: Accessible.Button
        Accessible.name: p.t("Computer")
        Accessible.description: p.t("About This Computer")
        Accessible.onPressAction: p.activate("win.tool-open", "about")
        Rectangle { width: parent.width; height: 1; color: Theme.line }
        Icon { x: 6; anchors.verticalCenter: parent.verticalCenter; name: "computer"; size: 30; color: Theme.ink }
        Column {
            x: 46; anchors.verticalCenter: parent.verticalCenter
            width: parent.width - 52
            spacing: 5
            Text { text: p.t("Computer"); color: Theme.ink; font.pixelSize: 16; font.weight: Font.Bold }
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
        TapHandler { onTapped: p.activate("win.tool-open", "about") }
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
        readonly property bool selected: !root.model.page && root.model.selected === modelData.key
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
            label: p.t("Eject ") + row.modelData.label
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

    component ToolRow: Rectangle {
        id: tool
        required property var modelData
        x: 12
        width: root.width - 24
        height: 38
        radius: 10
        color: modelData.selected ? Theme.alpha(Theme.accent, 0.16) : (toolHover.hovered ? Theme.hover : "transparent")
        Accessible.role: Accessible.Button
        Accessible.name: modelData.label
        Accessible.onPressAction: tool.go()
        // Tools open a page; Bookmarks rows pass their own action (P7-T08).
        function go() { p.activate(modelData.action || "win.tool-open", modelData.param !== undefined ? modelData.param : modelData.id) }
        Icon { x: 10; anchors.verticalCenter: parent.verticalCenter; name: tool.modelData.icon; size: 20; color: Theme.accentText }
        Text {
            x: 42; anchors.verticalCenter: parent.verticalCenter
            text: tool.modelData.label
            color: Theme.ink
            font.pixelSize: 15
            font.weight: tool.modelData.selected ? Font.Bold : Font.Normal
        }
        Text {
            visible: tool.modelData.count !== undefined && tool.modelData.count !== null
            anchors.right: parent.right; anchors.rightMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            text: tool.modelData.count === undefined || tool.modelData.count === null ? "" : tool.modelData.count
            color: Theme.faint
            font.pixelSize: 14
        }
        HoverHandler { id: toolHover }
        TapHandler { onTapped: tool.go() }
    }

    component WorkspaceRow: Rectangle {
        id: wsRow
        required property var modelData
        x: 12
        width: root.width - 24
        height: 38
        radius: 10
        color: modelData.open ? Theme.alpha(Theme.accent, 0.16) : (wsHover.hovered ? Theme.hover : "transparent")
        Accessible.role: Accessible.Button
        Accessible.name: modelData.name
        Accessible.onPressAction: p.activate("win.workspace-open", String(modelData.id))
        MiniFolder { x: 9; anchors.verticalCenter: parent.verticalCenter; size: 22; kind: wsRow.modelData.color }
        Text { x: 42; anchors.verticalCenter: parent.verticalCenter; width: parent.width - 52; elide: Text.ElideRight
               text: wsRow.modelData.name; color: Theme.ink; font.pixelSize: 15
               font.weight: wsRow.modelData.open ? Font.Bold : Font.Normal }
        HoverHandler { id: wsHover }
        FolderDrop { id: wsDrop; anchors.fill: parent; target: "workspace:" + wsRow.modelData.id }
        Rectangle { anchors.fill: parent; radius: parent.radius; color: "transparent"; border.color: Theme.accent
                    border.width: 2; visible: wsDrop.containsDrag }
        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            onClicked: mouse => {
                if (mouse.button === Qt.RightButton) p.activate("win.workspace-edit-ask", String(wsRow.modelData.id))
                else p.activate("win.workspace-open", String(wsRow.modelData.id))
            }
        }
    }

    Menu {
        id: menu
        property string key: ""
        MenuItem { text: p.t("Move Up"); onTriggered: p.activate("win.sidebar-move-up", menu.key) }
        MenuItem { text: p.t("Move Down"); onTriggered: p.activate("win.sidebar-move-down", menu.key) }
        MenuItem { text: p.t("Hide"); onTriggered: p.activate("win.sidebar-hide", menu.key) }
    }
}
