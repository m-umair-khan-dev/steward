import QtQuick
import QtQuick.Window
import dev.steward.files

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
    // Started at login in the background (P8-T03): no window until asked.
    visible: Qt.application.arguments.indexOf("--background") < 0
    title: p.t("Steward")
    flags: Qt.Window | Qt.FramelessWindowHint
    color: Theme.bg

    Presenter {
        id: p
        onQuitting: Qt.quit()
        // Test1.Screenshot; "shelf:PATH" captures the Shelf window instead.
        onShoot: path => {
            const shelfShot = path.startsWith("shelf:")
            const file = shelfShot ? path.slice(6) : path
            const item = shelfShot ? shelfWindow.contentItem : win.contentItem
            item.grabToImage(result => p.shot(path, result.saveToFile(file)))
        }
        onToast: (text, undo) => toast.show(text, undo)
        onToastundo: (text, action, param) => toast.show(text, true, action, param)
        onFocus: target => {
            if (target === "hide") {
                win.hide()
            } else if (target === "raise") {
                // The Shelf, the tray or a search result asked for the window.
                if (!win.visible) win.show()
                win.raise()
                win.requestActivate()
            } else if (target === "path") {
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
        // "null" (no dialog open) counts as missing: dialogs read fields of it.
        try { const value = JSON.parse(text); return value === null ? fallback : value } catch (e) { return fallback }
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
    readonly property var pane: parse(p.pane, ({ open: false }))
    readonly property var workspace: parse(p.workspace, ({ open: false }))
    readonly property var tools: parse(p.tools, ({ page: "" }))
    // A text field has the keyboard: shortcuts it needs (copy, paste, undo,
    // Delete) are left to it.
    readonly property bool typing: !!activeFocusItem && activeFocusItem.cursorPosition !== undefined

    // Style.
    // The system's dark mode: Qt 6.5+ reports it; on Qt 6.4 the controller
    // reads GNOME's own setting (prefs.systemDark).
    readonly property bool systemDark: Qt.styleHints.colorScheme !== undefined
                                       ? Qt.styleHints.colorScheme === 2 /* Qt.ColorScheme.Dark */
                                       : !!win.prefs.systemDark

    // Feature check (Ubuntu 24.04 has Qt 6.4): can QtQuick.Effects be used?
    // Probed once at start-up; STEWARD_LEGACY_FX=1 (prefs.legacyFx) forces the
    // fallback so it can be tested on newer Qt.
    property bool effectsAvailable: false
    function probeEffects() {
        try {
            Qt.createQmlObject("import QtQuick; import QtQuick.Effects; MultiEffect {}", win, "effects-probe").destroy()
            return true
        } catch (error) {
            console.info("QtQuick.Effects is not available; using the fallbacks")
            return false
        }
    }
    Binding { target: Theme; property: "effects"; value: win.effectsAvailable && !win.prefs.legacyFx }
    Binding {
        target: Theme; property: "dark"
        value: win.prefs.colorScheme === "dark" || (win.prefs.colorScheme !== "light" && win.systemDark)
    }
    // Style "System" takes Ubuntu's accent colour too; Light and Dark use the
    // colour chosen in Preferences.
    Binding { target: Theme; property: "systemAccent"; value: (win.prefs.colorScheme || "system") === "system" || win.prefs.accent === "system" }
    Binding { target: Theme; property: "accentName"; value: win.prefs.accent && win.prefs.accent !== "system" ? win.prefs.accent : "steward" }
    Binding { target: Theme; property: "motion"; value: win.prefs.motion === true }
    // Ubuntu's own accent setting first; the platform palette where it has none.
    Binding { target: Theme; property: "systemAccentColor"; value: win.prefs.systemAccentColor || win.palette.highlight }
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
    Component.onCompleted: { effectsAvailable = probeEffects(); report(); p.darkmode(Theme.dark) }

    // Keyboard shortcuts from the one table (actions.rs).
    Instantiator {
        model: win.accels
        delegate: Shortcut {
            required property var modelData
            sequences: modelData.keys
            context: Qt.ApplicationShortcut
            // Paused while Preferences records a new shortcut (P7-T11).
            enabled: !(modelData.typing && win.typing) && prefsDialog.recording === ""
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
            onKeepOpen: p.activate("win.toggle-sidebar", "")
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
            tool: ({ clean: p.t("Free Up Space"), installed: p.t("Installed"), about: p.t("About This Computer"),
                     agents: p.t("AI Agents"), markdown: (win.tools.markdown || {}).name || "",
                     bookmarks: p.t("Bookmarks") })[win.tools.page] || ""
        }
        PathBar {
            id: pathBar
            visible: !win.tools.page
            z: 5
            anchors.top: header.bottom
            width: parent.width
            model: win.pathbar
        }

        Item {
            id: body
            anchors.top: win.tools.page ? header.bottom : pathBar.bottom
            anchors.bottom: win.tools.page ? parent.bottom : statusBar.top
            width: parent.width

            readonly property bool showPreview: !!win.preview.visible && !win.pane.open
            readonly property int previewWidth: showPreview ? Math.min(320, Math.max(260, body.width * 0.26)) : 0

            FocusScope {
                id: content
                width: win.pane.open ? Math.round(parent.width / 2) : parent.width - body.previewWidth
                height: parent.height
                focus: true
                Behavior on width { enabled: Theme.motion; NumberAnimation { duration: 220; easing.type: Easing.OutCubic } }

                Keys.onPressed: event => {
                    // Dual pane: keys go to the active pane.
                    if (win.pane.open && win.pane.rightActive) {
                        const paneKeys = { [Qt.Key_Up]: "up", [Qt.Key_Down]: "down", [Qt.Key_Home]: "home",
                                           [Qt.Key_End]: "end", [Qt.Key_Return]: "return", [Qt.Key_Enter]: "return",
                                           [Qt.Key_Backspace]: "back" }
                        if (event.key in paneKeys) {
                            p.activate("win.pane-key", paneKeys[event.key])
                            event.accepted = true
                            return
                        }
                    }
                    const keys = { [Qt.Key_Left]: "left", [Qt.Key_Right]: "right", [Qt.Key_Up]: "up",
                                   [Qt.Key_Down]: "down", [Qt.Key_Home]: "home", [Qt.Key_End]: "end" }
                    if (event.key in keys && !(event.modifiers & Qt.AltModifier)) {
                        p.arrow(keys[event.key], !!(event.modifiers & Qt.ShiftModifier))
                        event.accepted = true
                    } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                        p.activate("win.open-selected", "")
                        event.accepted = true
                    } else if (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && (event.modifiers & Qt.ShiftModifier))) {
                        // The menu key and Shift+F10: the selection's menu, else the folder's.
                        // Opened from the keyboard: near the top of the folder, not at
                        // the window's corner.
                        if (win.selectedCount > 0) itemMenu.popup(content, 40, 60)
                        else backgroundMenu.popup(content, 40, 60)
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

                PaneHeader {
                    id: leftHeader
                    width: parent.width
                    visible: !!win.pane.open
                    crumbs: win.pane.leftCrumbs || []
                    summary: win.pane.leftSummary || ""
                    active: !win.pane.rightActive
                    onCrumb: path => p.activate("win.go-to", path)
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
                    y: leftHeader.visible ? leftHeader.height : 0
                    visible: !win.pane.open && !win.ops.trash && (win.folder.page === "grid" || win.folder.page === "list")
                    chips: win.folder.chips || []
                    summary: win.folder.summary || ""
                }

                FolderGrid {
                    anchors.top: leftHeader.visible ? leftHeader.bottom : (chips.visible ? chips.bottom : (trashBar.visible ? trashBar.bottom : parent.top))
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
                    anchors.top: leftHeader.visible ? leftHeader.bottom : (chips.visible ? chips.bottom : (trashBar.visible ? trashBar.bottom : parent.top))
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

            // A workspace page covers the folder while it is open.
            WorkspacePage {
                z: 20
                anchors.fill: parent
                visible: !!win.workspace.open
                model: win.workspace
            }

            // Tool pages (Phase 5) cover the folder too.
            CleanPage {
                z: 21
                anchors.fill: parent
                visible: win.tools.page === "clean"
                model: win.tools.clean || ({ cards: [], groups: [], disk: { looking: true }, admin: { rows: [] } })
            }

            InstalledPage {
                z: 21
                anchors.fill: parent
                visible: win.tools.page === "installed"
                model: win.tools.installed || ({ looking: true, tools: [], filters: [], packages: [] })
            }

            AgentsPage {
                z: 21
                anchors.fill: parent
                visible: win.tools.page === "agents"
                model: win.tools.agents || ({ looking: true, servers: [], projects: [] })
            }

            BookmarksPage {
                z: 21
                anchors.fill: parent
                visible: win.tools.page === "bookmarks"
                model: win.tools.bookmarks || ({ cards: [], tags: [] })
            }

            MarkdownPage {
                z: 21
                anchors.fill: parent
                visible: win.tools.page === "markdown"
                model: win.tools.markdown || ({ name: "" })
            }

            AboutPage {
                z: 21
                anchors.fill: parent
                visible: win.tools.page === "about"
                model: win.tools.about || ({ looking: true, sections: [] })
            }

            RightPane {
                visible: !!win.pane.open
                x: content.width
                width: parent.width - content.width
                height: parent.height
                model: win.pane
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
            visible: !win.tools.page
            anchors.bottom: parent.bottom
            width: parent.width
            height: win.pane.open ? 50 : 30
            color: Theme.pane
            Rectangle { width: parent.width; height: 1; color: Theme.line }
            // Dual pane: copy and move across, with their keys (reference footer).
            Row {
                visible: !!win.pane.open
                x: 14; anchors.verticalCenter: parent.verticalCenter
                spacing: 8
                PillButton { implicitHeight: 30; primary: true
                             text: p.t("Copy to “") + (win.pane.target || "") + "”   F5"
                             onClicked: p.activate("win.copy-across", "") }
                PillButton { implicitHeight: 30; text: p.t("Move   F6"); onClicked: p.activate("win.move-across", "") }
            }
            Text {
                visible: !!win.pane.open
                anchors.right: parent.right; anchors.rightMargin: 18; anchors.verticalCenter: parent.verticalCenter
                text: p.t("Tab switches pane · drag to move, Ctrl-drag to copy")
                color: Theme.muted; font.pixelSize: 12
            }
            Text {
                visible: !win.pane.open
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
    // The folder an item menu is about (one selected folder), else "".
    function menuFolderPath() {
        return win.folder.selectedFolder || ""
    }

    function itemMenuAt(pos) {
        if (!win.selection[pos]) p.pick(pos, 0)
        content.forceActiveFocus()
        itemMenu.popup()
    }
    readonly property int selectedCount: Object.keys(win.selection).length
    MenuPopup {
        id: itemMenu
        model: win.ops.trash ? [
            { text: p.t("Put Back"), action: "win.restore", param: "" }
        ] : [
            { text: p.t("Open"), action: "win.open-selected", param: "" },
            { text: p.t("Open With…"), action: "win.open-with", param: "", enabled: win.selectedCount === 1 },
            { separator: true },
            { text: p.t("Rename…"), action: "win.rename", param: "", enabled: win.selectedCount === 1 },
            { text: p.t("Duplicate"), action: "win.duplicate", param: "" },
            { text: p.t("Symbol and Colour…"), action: "win.folder-style-ask", param: "", enabled: win.selectedCount === 1 },
            { text: p.t("Open in Terminal"), action: "win.open-terminal", param: win.menuFolderPath(), enabled: win.menuFolderPath() !== "" },
            { text: p.t("Open in Editor"), action: "win.open-editor", param: win.menuFolderPath(), enabled: win.menuFolderPath() !== "" },
            { separator: true },
            { text: p.t("Add to Holding"), action: "win.shelf-hold", param: "" },
            { text: p.t("Pin to Shelf"), action: "win.shelf-pin", param: win.menuFolderPath(), enabled: win.menuFolderPath() !== "" },
            { separator: true },
            { text: p.t("Copy"), action: "win.copy", param: "" },
            { text: p.t("Cut"), action: "win.cut", param: "" },
            { separator: true },
            { text: p.t("Move to Trash"), action: "win.trash", param: "" }
        ].concat((win.sidebar.workspaces || []).length ? [{ separator: true }] : [])
         .concat((win.sidebar.workspaces || []).map(w => ({ text: p.t("Add to “") + w.name + "”", action: "win.workspace-add",
                                                            param: String(w.id) })))
    }
    MenuPopup {
        id: backgroundMenu
        model: win.ops.trash ? [
            { text: p.t("Empty Trash…"), action: "win.empty-trash", param: "" },
            { text: p.t("Select All"), action: "win.select-all", param: "" }
        ] : [
            { text: p.t("New Folder"), action: "win.new-folder", param: "" },
            { text: p.t("Paste"), action: "win.paste", param: "" },
            { text: p.t("Open in Terminal"), action: "win.open-terminal", param: "" },
            { text: p.t("Open in Editor"), action: "win.open-editor", param: "" },
            { separator: true },
            { text: p.t("Select All"), action: "win.select-all", param: "" },
            { text: p.t("Show Hidden Files"), action: "win.show-hidden", param: "", checked: !!win.prefs.showHidden }
        ]
    }

    // --- search and Peek (Phase 3) ---------------------------------------------
    SearchOverlay { id: searchOverlay; model: win.search }
    PeekOverlay { id: peekOverlay; model: win.peek }

    // --- dialogs ---------------------------------------------------------------
    AboutDialog { visible: p.dialog === "about"; version: p.version }
    WelcomeDialog { visible: p.dialog === "welcome"; info: win.dialogData; prefs: win.prefs; cardWidth: 640 }
    WhatsNewDialog { visible: p.dialog === "whats-new"; info: win.dialogData }
    StopServerDialog { visible: p.dialog === "stop-server"; info: win.dialogData }
    // The Shelf: a window of its own (P8-T01).
    ShelfWindow { id: shelfWindow; model: (win.tools || {}).shelf || ({ open: false }) }
    AddLinkDialog { visible: p.dialog === "bookmark-add"; info: win.dialogData }
    BookmarkDialog { visible: p.dialog === "bookmark-edit"; info: win.dialogData }
    CollectionDialog { visible: p.dialog === "collection-new"; info: win.dialogData }
    PreferencesDialog { id: prefsDialog; visible: p.dialog === "preferences"; prefs: win.prefs; accels: win.accels }
    ShortcutsDialog { visible: p.dialog === "shortcuts"; accels: win.accels }
    OpenWithDialog { visible: p.dialog === "open-with"; info: win.dialogData }
    ClashDialog { visible: p.dialog === "clash"; info: win.dialogData }
    EmptyTrashDialog { visible: p.dialog === "empty-trash"; info: win.dialogData }
    TidyDialog { visible: p.dialog === "tidy"; info: win.dialogData }
    WorkspaceDialog { visible: p.dialog === "workspace-new"; info: win.dialogData; editing: false }
    FolderStyleDialog { visible: p.dialog === "folder-style"; info: win.dialogData }
    WorkspaceDialog { visible: p.dialog === "workspace-edit"; info: win.dialogData; editing: true }
}
