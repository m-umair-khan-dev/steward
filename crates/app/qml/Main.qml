import QtQuick
import QtQuick.Window
import dev.cairn.files

// The main window (DESIGN.md §3.1, ADR 0008): inset sidebar (or icon rail
// that slides the sidebar out on hover), header, path bar, kind chips, the
// folder in a grid or list, and the preview pane. All state comes from the
// presenter as JSON; every command goes back through it.
Window {
    id: win
    width: 1280
    height: 800
    minimumWidth: 760
    minimumHeight: 480
    visible: true
    title: "Cairn"
    flags: Qt.Window | Qt.FramelessWindowHint
    color: Theme.bg

    Presenter {
        id: p
        onQuitting: Qt.quit()
        onShoot: path => win.contentItem.grabToImage(result => p.shot(path, result.saveToFile(path)))
        onToast: (text, undo) => toast.show(text, undo)
        onFocus: target => {
            if (target === "path") {
                pathBar.field.text = win.pathbar.text || ""
                pathBar.field.selectAll()
                pathBar.field.forceActiveFocus()
            } else if (target === "search") {
                searchOverlay.focusField()
            } else if (target === "peek") {
                peekOverlay.forceActiveFocus()
            } else if (target === "content") {
                content.forceActiveFocus()
            } else if (target === "filter") {
                header.filterField.selectAll()
                header.filterField.forceActiveFocus()
            }
        }
        onDialogChanged: if (dialog !== "") p.shown(dialog)
    }

    function parse(text, fallback) {
        if (!text) return fallback
        try { return JSON.parse(text) } catch (e) { return fallback }
    }
    readonly property var folder: parse(p.folder, ({}))
    readonly property var rows: parse(p.rows, [])
    readonly property var items: parse(p.items, [])
    readonly property var selection: parse(p.selection, ({}))
    readonly property var thumbs: parse(p.thumbs, ({}))
    readonly property var counts: parse(p.counts, ({}))
    readonly property var sidebar: parse(p.sidebar, ({ entries: [], expanded: true }))
    readonly property var preview: parse(p.preview, ({ visible: false, mode: "empty" }))
    readonly property var pathbar: parse(p.pathbar, ({ segments: [], editing: false, text: "" }))
    readonly property var nav: parse(p.nav, ({}))
    readonly property var prefs: parse(p.prefs, ({}))
    readonly property var accels: parse(p.accels, [])
    readonly property var dialogData: parse(p.dialogdata, ({}))
    readonly property var ops: parse(p.ops, ({ jobs: [], trash: false }))
    readonly property var search: parse(p.search, ({ open: false }))
    readonly property var peek: parse(p.peek, ({ open: false }))
    // A text field has the keyboard: shortcuts it needs (copy, paste, undo,
    // Delete) are left to it.
    readonly property bool typing: !!activeFocusItem && activeFocusItem.cursorPosition !== undefined

    // Style.
    readonly property bool systemDark: Qt.styleHints.colorScheme === Qt.ColorScheme.Dark
    Binding {
        target: Theme; property: "dark"
        value: win.prefs.colorScheme === "dark" || (win.prefs.colorScheme !== "light" && win.systemDark)
    }
    Binding { target: Theme; property: "systemAccent"; value: win.prefs.accent === "system" }
    Binding { target: Theme; property: "motion"; value: win.prefs.motion === true }
    Binding { target: Theme; property: "systemAccentColor"; value: win.palette.highlight }
    Connections {
        target: Theme
        function onDarkChanged() { p.darkmode(Theme.dark) }
    }

    // Window size: restored once, reported on change.
    property bool sized: false
    onPrefsChanged: {
        if (!sized && prefs.width > 0) {
            sized = true
            width = Math.max(prefs.width, minimumWidth)
            height = Math.max(prefs.height, minimumHeight)
            if (prefs.maximized) showMaximized()
        }
    }
    function report() { p.window(width, height, active) }
    onWidthChanged: report()
    onHeightChanged: report()
    onActiveChanged: report()
    Component.onCompleted: { report(); p.darkmode(Theme.dark) }

    // Keyboard shortcuts from the one table (actions.rs).
    Instantiator {
        model: win.accels
        delegate: Shortcut {
            required property var modelData
            sequences: modelData.keys
            context: Qt.ApplicationShortcut
            enabled: !(modelData.typing && win.typing)
            onActivated: p.activate(modelData.action, modelData.param)
        }
    }

    // Resize from the window edges (no system frame).
    MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton
        z: -1
        readonly property int edge: 6
        function edges(x, y) {
            let e = 0
            if (x < edge) e |= Qt.LeftEdge
            if (x > width - edge) e |= Qt.RightEdge
            if (y < edge) e |= Qt.TopEdge
            if (y > height - edge) e |= Qt.BottomEdge
            return e
        }
        cursorShape: {
            const e = edges(mouseX, mouseY)
            if (e === (Qt.LeftEdge | Qt.TopEdge) || e === (Qt.RightEdge | Qt.BottomEdge)) return Qt.SizeFDiagCursor
            if (e === (Qt.RightEdge | Qt.TopEdge) || e === (Qt.LeftEdge | Qt.BottomEdge)) return Qt.SizeBDiagCursor
            if (e & (Qt.LeftEdge | Qt.RightEdge)) return Qt.SizeHorCursor
            if (e & (Qt.TopEdge | Qt.BottomEdge)) return Qt.SizeVerCursor
            return Qt.ArrowCursor
        }
        onPressed: mouse => {
            const e = edges(mouse.x, mouse.y)
            if (e) win.startSystemResize(e)
            else mouse.accepted = false
        }
    }

    // --- sidebar -------------------------------------------------------------
    readonly property bool expanded: sidebar.expanded !== false
    readonly property int sideWidth: 262

    Sidebar {
        id: side
        x: 8; y: 8
        width: win.sideWidth
        height: win.height - 16
        visible: win.expanded
        model: win.sidebar
        onCollapse: p.activate("win.toggle-sidebar", "")
    }

    Rail {
        id: rail
        x: 8; y: 8
        height: win.height - 16
        visible: !win.expanded
        model: win.sidebar
        onExpand: p.activate("win.toggle-sidebar", "")
        onHoveredChanged_: hovered => { if (hovered) peek.open = true }
    }

    // The sidebar sliding over the content while the pointer is on the rail.
    Item {
        id: peek
        property bool open: false
        visible: !win.expanded && x > -width
        x: open ? 8 : -win.sideWidth - 20
        y: 8
        z: 50
        width: win.sideWidth
        height: win.height - 16
        Behavior on x { enabled: Theme.motion; NumberAnimation { duration: 250; easing.type: Easing.OutCubic } }
        Accessible.ignored: !open
        HoverHandler { id: peekHover; onHoveredChanged: if (!hovered) peekClose.restart() }
        Timer { id: peekClose; interval: 200; onTriggered: if (!peekHover.hovered) peek.open = false }
        Sidebar {
            anchors.fill: parent
            overlay: true
            model: win.sidebar
            layer.enabled: true
        }
    }
    onExpandedChanged: peek.open = false

    // --- content -------------------------------------------------------------
    Rectangle {
        id: pane
        x: win.expanded ? side.width + 16 : rail.width + 16
        y: 0
        width: win.width - x
        height: win.height
        color: Theme.pane
        Behavior on x { enabled: Theme.motion; NumberAnimation { duration: 220; easing.type: Easing.OutCubic } }

        Header {
            id: header
            width: parent.width
            folder: win.folder
            nav: win.nav
            prefs: win.prefs
            ops: win.ops
        }
        PathBar {
            id: pathBar
            z: 5
            anchors.top: header.bottom
            width: parent.width
            model: win.pathbar
        }

        Item {
            id: body
            anchors.top: pathBar.bottom
            anchors.bottom: statusBar.top
            width: parent.width

            readonly property bool showPreview: !!win.preview.visible
            readonly property int previewWidth: showPreview ? Math.min(320, Math.max(260, body.width * 0.26)) : 0

            FocusScope {
                id: content
                width: parent.width - body.previewWidth
                height: parent.height
                focus: true
                Behavior on width { enabled: Theme.motion; NumberAnimation { duration: 220; easing.type: Easing.OutCubic } }

                Keys.onPressed: event => {
                    const keys = { [Qt.Key_Left]: "left", [Qt.Key_Right]: "right", [Qt.Key_Up]: "up",
                                   [Qt.Key_Down]: "down", [Qt.Key_Home]: "home", [Qt.Key_End]: "end" }
                    if (event.key in keys && !(event.modifiers & Qt.AltModifier)) {
                        p.arrow(keys[event.key], !!(event.modifiers & Qt.ShiftModifier))
                        event.accepted = true
                    } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                        p.activate("win.open-selected", "")
                        event.accepted = true
                    } else if (event.key === Qt.Key_A && (event.modifiers & Qt.ControlModifier)) {
                        p.activate("win.select-all", "")
                        event.accepted = true
                    }
                }
                // The background: a click clears the selection, a right-click
                // opens the folder's menu. Cards stop clicks themselves.
                FolderDrop { anchors.fill: parent; z: -2; enabled: !win.ops.trash }
                MouseArea {
                    anchors.fill: parent
                    z: -1
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    onClicked: mouse => {
                        content.forceActiveFocus()
                        if (mouse.button === Qt.RightButton) backgroundMenu.popup()
                        else p.pick(-1, 0)
                    }
                }

                TrashBar {
                    id: trashBar
                    width: parent.width
                    visible: !!win.ops.trash
                    hasSelection: Object.keys(win.selection).length > 0
                    empty: (win.folder.chips || []).length === 0
                }
                ChipRow {
                    id: chips
                    width: parent.width
                    visible: !win.ops.trash && (win.folder.page === "grid" || win.folder.page === "list")
                    chips: win.folder.chips || []
                    summary: win.folder.summary || ""
                }

                FolderGrid {
                    anchors.top: chips.visible ? chips.bottom : (trashBar.visible ? trashBar.bottom : parent.top)
                    anchors.bottom: parent.bottom
                    width: parent.width
                    visible: win.folder.page === "grid"
                    rows: win.rows
                    selection: win.selection
                    thumbs: win.thumbs
                    counts: win.counts
                    metrics: win.folder.metrics || ({ folder: 168, file: 240, gap: 18 })
                    groupBy: win.folder.groupBy || "none"
                    renaming: win.ops.renaming || ""
                    anchor: win.folder.anchor === undefined ? -1 : win.folder.anchor
                    previewShown: !!win.preview.visible
                    onItemMenu: pos => win.itemMenuAt(pos)
                }
                FolderList {
                    anchors.top: chips.visible ? chips.bottom : (trashBar.visible ? trashBar.bottom : parent.top)
                    anchors.bottom: parent.bottom
                    width: parent.width
                    visible: win.folder.page === "list"
                    items: visible ? win.items : []
                    selection: win.selection
                    anchor: win.folder.anchor === undefined ? -1 : win.folder.anchor
                    renaming: win.ops.renaming || ""
                    onItemMenu: pos => win.itemMenuAt(pos)
                }
                StatusPage {
                    anchors.fill: parent
                    visible: !!win.folder.page && win.folder.page !== "grid" && win.folder.page !== "list"
                    loading: win.folder.page === "loading"
                    icon: win.folder.statusIcon || "folder"
                    title: win.folder.statusTitle || ""
                    text: win.folder.statusText || ""
                }
            }

            PreviewPane {
                anchors.right: parent.right
                width: Math.max(body.previewWidth, 0)
                height: parent.height
                visible: width > 1
                model: win.preview
                clip: true
            }
        }

        Rectangle {
            id: statusBar
            anchors.bottom: parent.bottom
            width: parent.width
            height: 30
            color: Theme.pane
            Rectangle { width: parent.width; height: 1; color: Theme.line }
            Text {
                x: 22; anchors.verticalCenter: parent.verticalCenter
                text: win.folder.status || ""
                color: Theme.muted
                font.pixelSize: 12
                Accessible.role: Accessible.StatusBar
                Accessible.name: text
            }
        }

        Toast { id: toast }
    }

    // --- context menus (P2-T10) ---------------------------------------------------
    function itemMenuAt(pos) {
        if (!win.selection[pos]) p.pick(pos, 0)
        content.forceActiveFocus()
        itemMenu.popup()
    }
    readonly property int selectedCount: Object.keys(win.selection).length
    MenuPopup {
        id: itemMenu
        model: win.ops.trash ? [
            { text: "Put Back", action: "win.restore", param: "" }
        ] : [
            { text: "Open", action: "win.open-selected", param: "" },
            { text: "Open With…", action: "win.open-with", param: "", enabled: win.selectedCount === 1 },
            { separator: true },
            { text: "Rename…", action: "win.rename", param: "", enabled: win.selectedCount === 1 },
            { text: "Duplicate", action: "win.duplicate", param: "" },
            { separator: true },
            { text: "Copy", action: "win.copy", param: "" },
            { text: "Cut", action: "win.cut", param: "" },
            { separator: true },
            { text: "Move to Trash", action: "win.trash", param: "" }
        ]
    }
    MenuPopup {
        id: backgroundMenu
        model: win.ops.trash ? [
            { text: "Empty Trash…", action: "win.empty-trash", param: "" },
            { text: "Select All", action: "win.select-all", param: "" }
        ] : [
            { text: "New Folder", action: "win.new-folder", param: "" },
            { text: "Paste", action: "win.paste", param: "" },
            { separator: true },
            { text: "Select All", action: "win.select-all", param: "" },
            { text: "Show Hidden Files", action: "win.show-hidden", param: "", checked: !!win.prefs.showHidden }
        ]
    }

    // --- search and Peek (Phase 3) ---------------------------------------------
    SearchOverlay { id: searchOverlay; model: win.search }
    PeekOverlay { id: peekOverlay; model: win.peek }

    // --- dialogs ---------------------------------------------------------------
    AboutDialog { visible: p.dialog === "about"; version: p.version }
    PreferencesDialog { visible: p.dialog === "preferences"; prefs: win.prefs }
    ShortcutsDialog { visible: p.dialog === "shortcuts"; accels: win.accels }
    OpenWithDialog { visible: p.dialog === "open-with"; info: win.dialogData }
    ClashDialog { visible: p.dialog === "clash"; info: win.dialogData }
    EmptyTrashDialog { visible: p.dialog === "empty-trash"; info: win.dialogData }
}
