import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Effects

// The header (DESIGN.md §3.3; reference smart-downloads.png): back/forward
// pill, folder title, grouping and view controls, preview toggle, filter
// field, menu and window buttons. Empty areas move the window.
Rectangle {
    id: root
    property var folder: ({})
    property var nav: ({})
    property var prefs: ({})
    property var ops: ({ jobs: [] })
    property alias filterField: filter
    height: 60
    color: Theme.pane
    Accessible.role: Accessible.ToolBar
    Accessible.name: "Main toolbar"

    DragHandler { target: null; onActiveChanged: if (active) root.Window.window.startSystemMove() }
    TapHandler {
        onDoubleTapped: {
            const w = root.Window.window
            w.visibility = w.visibility === Window.Maximized ? Window.Windowed : Window.Maximized
        }
    }

    Row {
        id: left
        x: 16
        anchors.verticalCenter: parent.verticalCenter
        spacing: 14
        Rectangle {
            width: 78; height: 38; radius: 19
            color: Theme.pane
            border.color: Theme.line
            layer.enabled: true
            layer.effect: MultiEffect { shadowEnabled: true; shadowOpacity: 0.10; shadowBlur: 0.5; shadowVerticalOffset: 1 }
            Row {
                anchors.centerIn: parent
                IconButton { width: 36; height: 32; flat: true; icon: "back"; label: "Back"
                             enabled: !!root.nav.back; onClicked: p.activate("win.back", "") }
                IconButton { width: 36; height: 32; flat: true; icon: "forward"; label: "Forward"
                             enabled: !!root.nav.forward; onClicked: p.activate("win.forward", "") }
            }
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.folder.title || ""
            color: Theme.ink
            font.pixelSize: 20
            font.weight: Font.Bold
            elide: Text.ElideRight
            // Room left of the controls: margins, the nav pill and the spacing.
            width: Math.max(0, Math.min(implicitWidth, root.width - right.width - 16 - 78 - 14 - 24))
        }
    }

    Row {
        id: right
        anchors.right: parent.right
        anchors.rightMargin: 12
        anchors.verticalCenter: parent.verticalCenter
        spacing: 8

        Segmented {
            name: "Group by"
            options: [{ id: "none", label: "None" }, { id: "kind", label: "Kind" }, { id: "created", label: "Date" }]
            current: root.folder.groupBy || "none"
            onPicked: id => p.activate("win.group-by", id)
        }
        Segmented {
            name: "View"
            options: [{ id: "list", icon: "list", a11y: "List" }, { id: "grid", icon: "grid", a11y: "Grid" }]
            current: root.folder.viewMode || "grid"
            onPicked: id => p.activate("win.view-mode", id)
        }
        IconButton {
            icon: "sort"; label: "Sort and size"
            onClicked: sortMenu.popup()
            MenuPopup {
                id: sortMenu
                model: [
                    { text: "Name", action: "win.sort-by", param: "name", checked: root.folder.sort === "name" },
                    { text: "Size", action: "win.sort-by", param: "size", checked: root.folder.sort === "size" },
                    { text: "Modified", action: "win.sort-by", param: "modified", checked: root.folder.sort === "modified" },
                    { text: "Created", action: "win.sort-by", param: "created", checked: root.folder.sort === "created" },
                    { text: "Kind", action: "win.sort-by", param: "kind", checked: root.folder.sort === "kind" },
                    { separator: true },
                    { text: "Descending", action: "win.sort-descending", param: "", checked: !!root.folder.descending },
                    { separator: true },
                    { text: "Larger", action: "win.zoom-in", param: "" },
                    { text: "Smaller", action: "win.zoom-out", param: "" },
                    { text: "Normal Size", action: "win.zoom-reset", param: "" }
                ]
            }
        }
        IconButton {
            icon: "preview"; label: "Preview pane"
            checked: !!root.prefs.preview
            onClicked: p.activate("win.toggle-preview", "")
        }
        TextField {
            id: filter
            // Narrower on small windows, so the folder title keeps its room.
            width: root.width < 1100 ? 150 : 200
            height: 36
            placeholderText: "Filter this folder"
            placeholderTextColor: Theme.faint
            color: Theme.ink
            leftPadding: 34
            font.pixelSize: 13
            Accessible.name: "Filter this folder"
            background: Rectangle {
                radius: 18
                color: Theme.track
                border.color: filter.activeFocus ? Theme.accent : "transparent"
                border.width: 1.5
                Icon { x: 11; anchors.verticalCenter: parent.verticalCenter; name: "search"; size: 16; color: Theme.faint }
            }
            onTextChanged: p.filter(text)
            Keys.onEscapePressed: { text = ""; root.Window.window.contentItem.forceActiveFocus() }
        }
        // The queue (P2-T07): shows while something runs.
        Item {
            visible: (root.ops.jobs || []).length > 0
            width: visible ? 34 : 0
            height: 34
            IconButton {
                id: queueButton
                anchors.fill: parent
                icon: "progress"; label: "Operations"
                checked: jobs.opened
                onClicked: jobs.opened ? jobs.close() : jobs.open()
            }
            // Progress ring around the button.
            Canvas {
                anchors.fill: parent
                anchors.margins: -3
                property real fraction: root.ops.fraction || 0
                onFractionChanged: requestPaint()
                onPaint: {
                    const ctx = getContext("2d")
                    ctx.reset()
                    ctx.lineWidth = 3
                    ctx.strokeStyle = Theme.accent
                    ctx.beginPath()
                    ctx.arc(width / 2, height / 2, width / 2 - 2, -Math.PI / 2, -Math.PI / 2 + 2 * Math.PI * fraction)
                    ctx.stroke()
                }
            }
            JobsPopup {
                id: jobs
                y: parent.height + 8
                x: parent.width - width
                jobs: root.ops.jobs || []
            }
        }
        IconButton {
            icon: "more"; label: "Main menu"
            onClicked: mainMenu.popup()
            MenuPopup {
                id: mainMenu
                model: [
                    { text: root.ops.undo ? "Undo " + root.ops.undo : "Undo", action: "win.undo", param: "",
                      enabled: !!root.ops.undo },
                    { text: root.ops.redo ? "Redo " + root.ops.redo : "Redo", action: "win.redo", param: "",
                      enabled: !!root.ops.redo },
                    { separator: true },
                    { text: "New Folder", action: "win.new-folder", param: "", enabled: !root.ops.trash },
                    { text: "Paste", action: "win.paste", param: "", enabled: !root.ops.trash },
                    { text: "Select All", action: "win.select-all", param: "" },
                    { separator: true },
                    { text: "Show Hidden Files", action: "win.show-hidden", param: "", checked: !!root.prefs.showHidden },
                    { text: "Expand Sidebar", action: "win.toggle-sidebar", param: "", checked: !!root.prefs.sidebar },
                    { text: "Restore Sidebar", action: "win.sidebar-reset", param: "" },
                    { separator: true },
                    { text: "Follow System Style", action: "app.color-scheme", param: "system", checked: root.prefs.colorScheme === "system" },
                    { text: "Light", action: "app.color-scheme", param: "light", checked: root.prefs.colorScheme === "light" },
                    { text: "Dark", action: "app.color-scheme", param: "dark", checked: root.prefs.colorScheme === "dark" },
                    { separator: true },
                    { text: "Preferences", action: "app.preferences", param: "" },
                    { text: "Keyboard Shortcuts", action: "win.show-help-overlay", param: "" },
                    { text: "About Cairn", action: "app.about", param: "" },
                    { separator: true },
                    { text: "Quit", action: "app.quit", param: "" }
                ]
            }
        }
        Item { width: 4; height: 1 }
        // Window buttons (Ubuntu side: right).
        IconButton { width: 28; height: 28; iconSize: 14; icon: "minimize"; label: "Minimize"
                     onClicked: root.Window.window.showMinimized() }
        IconButton { width: 28; height: 28; iconSize: 13; icon: "maximize"; label: "Maximize"
                     onClicked: { const w = root.Window.window
                                  w.visibility = w.visibility === Window.Maximized ? Window.Windowed : Window.Maximized } }
        IconButton { width: 28; height: 28; iconSize: 14; icon: "close"; label: "Close"
                     onClicked: p.activate("app.quit", "") }
    }
}
