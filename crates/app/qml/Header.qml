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
    /// A tool page's title ("Free Up Space"): the folder controls hide.
    property string tool: ""
    height: 60
    color: Theme.pane
    Accessible.role: Accessible.ToolBar
    Accessible.name: p.t("Main toolbar")

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
                IconButton { width: 36; height: 32; flat: true; icon: "back"; label: p.t("Back")
                             enabled: !!root.nav.back; onClicked: p.activate("win.back", "") }
                IconButton { width: 36; height: 32; flat: true; icon: "forward"; label: p.t("Forward")
                             enabled: !!root.nav.forward; onClicked: p.activate("win.forward", "") }
            }
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.tool || root.folder.title || ""
            color: Theme.ink
            font.pixelSize: 21
            font.weight: Font.DemiBold
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
            visible: !root.tool
            name: p.t("Group by")
            options: [{ id: "none", label: p.t("None") }, { id: "kind", label: p.t("Kind") }, { id: "created", label: p.t("Date") }]
            current: root.folder.groupBy || "none"
            onPicked: id => p.activate("win.group-by", id)
        }
        Segmented {
            visible: !root.tool
            name: p.t("View")
            options: [{ id: "list", icon: "list", a11y: p.t("List") }, { id: "grid", icon: "grid", a11y: p.t("Grid") }]
            current: root.folder.viewMode || "grid"
            onPicked: id => p.activate("win.view-mode", id)
        }
        IconButton {
            visible: !root.tool
            icon: "sort"; label: p.t("Sort and size")
            onClicked: sortMenu.popup()
            MenuPopup {
                id: sortMenu
                model: [
                    { text: p.t("Name"), action: "win.sort-by", param: "name", checked: root.folder.sort === "name" },
                    { text: p.t("Size"), action: "win.sort-by", param: "size", checked: root.folder.sort === "size" },
                    { text: p.t("Modified"), action: "win.sort-by", param: "modified", checked: root.folder.sort === "modified" },
                    { text: p.t("Created"), action: "win.sort-by", param: "created", checked: root.folder.sort === "created" },
                    { text: p.t("Kind"), action: "win.sort-by", param: "kind", checked: root.folder.sort === "kind" },
                    { separator: true },
                    { text: p.t("Descending"), action: "win.sort-descending", param: "", checked: !!root.folder.descending },
                    { separator: true },
                    { text: p.t("Larger"), action: "win.zoom-in", param: "" },
                    { text: p.t("Smaller"), action: "win.zoom-out", param: "" },
                    { text: p.t("Normal Size"), action: "win.zoom-reset", param: "" }
                ]
            }
        }
        IconButton {
            visible: !root.tool
            icon: "preview"; label: p.t("Preview pane")
            checked: !!root.prefs.preview
            onClicked: p.activate("win.toggle-preview", "")
        }
        TextField {
            id: filter
            visible: !root.tool
            // Narrower on small windows, so the folder title keeps its room.
            width: root.width < 1100 ? 150 : 200
            height: 36
            placeholderText: p.t("Filter this folder")
            placeholderTextColor: Theme.faint
            color: Theme.ink
            leftPadding: 34
            font.pixelSize: 13
            Accessible.name: p.t("Filter this folder")
            background: Rectangle {
                radius: 18
                color: Theme.pane
                border.color: filter.activeFocus ? Theme.accent : Theme.line
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
                icon: "progress"; label: p.t("Operations")
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
            icon: "more"; label: p.t("Main menu")
            onClicked: mainMenu.popup()
            MenuPopup {
                id: mainMenu
                model: [
                    { text: root.ops.undo ? p.t("Undo ") + root.ops.undo : p.t("Undo"), action: "win.undo", param: "",
                      enabled: !!root.ops.undo },
                    { text: root.ops.redo ? p.t("Redo ") + root.ops.redo : p.t("Redo"), action: "win.redo", param: "",
                      enabled: !!root.ops.redo },
                    { separator: true },
                    { text: p.t("New Folder"), action: "win.new-folder", param: "", enabled: !root.ops.trash },
                    { text: p.t("Paste"), action: "win.paste", param: "", enabled: !root.ops.trash },
                    { text: p.t("Select All"), action: "win.select-all", param: "" },
                    { separator: true },
                    { text: p.t("Show Hidden Files"), action: "win.show-hidden", param: "", checked: !!root.prefs.showHidden },
                    { text: p.t("Expand Sidebar"), action: "win.toggle-sidebar", param: "", checked: !!root.prefs.sidebar },
                    { text: p.t("Restore Sidebar"), action: "win.sidebar-reset", param: "" },
                    { separator: true },
                    { text: p.t("Follow System Style"), action: "app.color-scheme", param: "system", checked: root.prefs.colorScheme === "system" },
                    { text: p.t("Light"), action: "app.color-scheme", param: "light", checked: root.prefs.colorScheme === "light" },
                    { text: p.t("Dark"), action: "app.color-scheme", param: "dark", checked: root.prefs.colorScheme === "dark" },
                    { separator: true },
                    { text: p.t("Preferences"), action: "app.preferences", param: "" },
                    { text: p.t("Keyboard Shortcuts"), action: "win.show-help-overlay", param: "" },
                    { text: p.t("About Cairn"), action: "app.about", param: "" },
                    { separator: true },
                    { text: p.t("Quit"), action: "app.quit", param: "" }
                ]
            }
        }
        Item { width: 4; height: 1 }
        // Window buttons (Ubuntu side: right).
        IconButton { width: 28; height: 28; iconSize: 14; icon: "minimize"; label: p.t("Minimize")
                     onClicked: root.Window.window.showMinimized() }
        IconButton { width: 28; height: 28; iconSize: 13; icon: "maximize"; label: p.t("Maximize")
                     onClicked: { const w = root.Window.window
                                  w.visibility = w.visibility === Window.Maximized ? Window.Windowed : Window.Maximized } }
        IconButton { width: 28; height: 28; iconSize: 14; icon: "close"; label: p.t("Close")
                     onClicked: p.activate("app.quit", "") }
    }
}
