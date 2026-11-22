import QtQuick
import QtQuick.Controls.Basic

// Preferences (P6-T03): General, Appearance, Shortcuts (the list of every
// key, from the same table that binds them) and Search and index.
Sheet {
    id: root
    property var prefs: ({})
    property var accels: []

    // Shortcuts (P7-T11): change, clear, restore defaults. While a new key is
    // being recorded, every app shortcut is paused (Main.qml).
    property string recording: ""
    property string recordingName: ""

    // Qt key-sequence text for a key press, e.g. "Ctrl+Shift+O".
    function sequence(event) {
        const named = {
            [Qt.Key_Left]: "Left", [Qt.Key_Right]: "Right", [Qt.Key_Up]: "Up", [Qt.Key_Down]: "Down",
            [Qt.Key_Home]: "Home", [Qt.Key_End]: "End", [Qt.Key_PageUp]: "PgUp", [Qt.Key_PageDown]: "PgDown",
            [Qt.Key_Delete]: "Del", [Qt.Key_Insert]: "Ins", [Qt.Key_Space]: "Space", [Qt.Key_Return]: "Return",
            [Qt.Key_Enter]: "Enter", [Qt.Key_Tab]: "Tab", [Qt.Key_Backspace]: "Backspace",
            [Qt.Key_Comma]: ",", [Qt.Key_Period]: ".", [Qt.Key_Slash]: "/", [Qt.Key_Minus]: "-",
            [Qt.Key_Equal]: "=", [Qt.Key_Plus]: "+", [Qt.Key_Semicolon]: ";", [Qt.Key_Apostrophe]: "'",
            [Qt.Key_BracketLeft]: "[", [Qt.Key_BracketRight]: "]", [Qt.Key_Backslash]: "\\",
            [Qt.Key_QuoteLeft]: "`", [Qt.Key_Question]: "?", [Qt.Key_Menu]: "Menu"
        }
        let key = named[event.key]
        if (!key && event.key >= Qt.Key_A && event.key <= Qt.Key_Z) key = String.fromCharCode(event.key)
        if (!key && event.key >= Qt.Key_0 && event.key <= Qt.Key_9) key = String.fromCharCode(event.key)
        if (!key && event.key >= Qt.Key_F1 && event.key <= Qt.Key_F35) key = "F" + (event.key - Qt.Key_F1 + 1)
        if (!key) return ""       // a modifier on its own, or a key we cannot name
        let parts = []
        if (event.modifiers & Qt.ControlModifier) parts.push("Ctrl")
        if (event.modifiers & Qt.AltModifier) parts.push("Alt")
        if (event.modifiers & Qt.ShiftModifier) parts.push("Shift")
        if (event.modifiers & Qt.MetaModifier) parts.push("Meta")
        parts.push(key)
        return parts.join("+")
    }

    property string page: "general"
    title: p.t("Preferences")
    cardWidth: 660
    cardHeight: 600
    onVisibleChanged: { if (visible) page = "general"; else recording = "" }

    // A terminal or editor chooser: the apps found, the system's default
    // (terminals), and "Your own command" with {} for the folder.
    component OpenInChoice: Column {
        id: openIn
        property string role: "terminal"
        property string label: ""
        readonly property var info: root.prefs.openIn || ({})
        readonly property var apps: (role === "terminal" ? info.terminals : info.editors) || []
        readonly property string chosen: (role === "terminal" ? info.terminal : info.editor) || ""
        readonly property var options: (role === "terminal" && info.systemTerminal
                                         ? [{ id: "", label: p.t("The system’s terminal") }] : [])
                                        .concat(apps)
                                        .concat([{ id: "custom", label: p.t("Your own command…") }])
        width: parent.width
        spacing: 4
        Item {
            width: parent.width; height: 48
            Text { anchors.verticalCenter: parent.verticalCenter; text: openIn.label; color: Theme.ink; font.pixelSize: 14 }
            ComboBox {
                id: combo
                anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                width: 240
                model: openIn.options
                textRole: "label"
                currentIndex: Math.max(0, openIn.options.findIndex(o => o.id === openIn.chosen))
                Accessible.name: openIn.label
                onActivated: index => p.setpref("open-in-" + openIn.role, openIn.options[index].id)
                // In the app's style: a white rounded box with a chevron.
                implicitHeight: 34
                background: Rectangle { radius: 10; color: Theme.pane; border.color: combo.activeFocus ? Theme.accent : Theme.line }
                contentItem: Text {
                    leftPadding: 12; rightPadding: 30
                    verticalAlignment: Text.AlignVCenter
                    text: combo.displayText; color: Theme.ink; font.pixelSize: 13; elide: Text.ElideRight
                }
                indicator: Icon { x: combo.width - width - 10; anchors.verticalCenter: parent.verticalCenter
                                  name: "chevron"; size: 14; rotation: 90; color: Theme.muted }
                popup.background: Rectangle { radius: 12; color: Theme.card; border.color: Theme.line }
                delegate: ItemDelegate {
                    required property var modelData
                    required property int index
                    width: combo.width
                    highlighted: combo.highlightedIndex === index
                    contentItem: Text { text: parent.modelData.label; color: Theme.ink; font.pixelSize: 13
                                        verticalAlignment: Text.AlignVCenter }
                    background: Rectangle { radius: 8; color: parent.highlighted ? Theme.hover : "transparent" }
                }
            }
        }
        TextField {
            visible: openIn.chosen === "custom"
            width: parent.width
            placeholderText: openIn.role === "terminal" ? p.t("e.g. foot -D {}") : p.t("e.g. hx {}")
            text: (openIn.role === "terminal" ? openIn.info.terminalCommand : openIn.info.editorCommand) || ""
            Accessible.name: openIn.label + p.t(": your own command")
            color: Theme.ink
            background: Rectangle { radius: 9; color: Theme.pane; border.color: Theme.line }
            onEditingFinished: p.setpref("open-in-" + openIn.role + "-command", text)
        }
    }

    // A switch with a line of explanation.
    component PrefSwitch: Item {
        id: ps
        property string label: ""
        property string note: ""
        property string key: ""
        property bool on: false
        width: parent.width; height: 70
        Column {
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - 70
            Text { text: ps.label; color: Theme.ink; font.pixelSize: 14 }
            Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.muted; font.pixelSize: 12; text: ps.note }
        }
        Switch {
            anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
            checked: ps.on
            Accessible.name: ps.label
            // Any change (a click, or an accessibility Toggle); writes only a real change.
            onCheckedChanged: if (checked !== ps.on) p.setpref(ps.key, checked ? "true" : "false")
        }
    }

    component Choice: Item {
        id: choice
        property string label: ""
        property string key: ""
        property var options: []
        width: parent.width
        height: 48
        Text { anchors.verticalCenter: parent.verticalCenter; text: choice.label; color: Theme.ink; font.pixelSize: 14 }
        Segmented {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            name: choice.label
            options: choice.options
            current: root.prefs[choice.key === "default-view" ? "defaultView" : choice.key === "row-size" ? "rowSize"
                                : choice.key === "color-scheme" ? "colorScheme" : "accent"] || ""
            onPicked: id => p.setpref(choice.key, id)
        }
    }

    // Takes the key press while recording.
    Item {
        id: recorder
        width: 1; height: 1
        Keys.onPressed: event => {
            if (root.recording === "") { event.accepted = false; return }
            event.accepted = true
            if (event.key === Qt.Key_Escape) { root.recording = ""; root.refocus(); return }
            const keys = root.sequence(event)
            if (keys === "") return
            // The Shelf's global key is a setting; the rest are app shortcuts.
            if (root.recording === "__shelf") p.setpref("shelf-shortcut", keys)
            else p.activate("win.shortcut-set", root.recording + "|" + keys)
            root.recording = ""
            root.refocus()
        }
    }
    Segmented {
        id: tabs
        name: p.t("Preferences pages")
        options: [{ id: "general", label: p.t("General") }, { id: "appearance", label: p.t("Appearance") },
                  { id: "shortcuts", label: p.t("Shortcuts") }, { id: "shelf", label: p.t("Shelf and Tray") },
                  { id: "search", label: p.t("Search and Index") }]
        current: root.page
        onPicked: id => root.page = id
    }

    Flickable {
        anchors.top: tabs.bottom; anchors.topMargin: 14
        width: parent.width
        height: parent.height - tabs.height - 14
        contentHeight: pages.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

        Item {
            id: pages
            width: parent.width
            implicitHeight: (general.visible ? general.implicitHeight : 0) + (appearance.visible ? appearance.implicitHeight : 0)
                            + (shortcuts.visible ? shortcuts.implicitHeight : 0) + (search.visible ? search.implicitHeight : 0)
                            + (shelfPage.visible ? shelfPage.implicitHeight : 0)

    Column {
        id: general
        visible: root.page === "general"
        width: parent.width
        spacing: 2
        Choice { label: p.t("Default view"); key: "default-view"
                 options: [{ id: "grid", label: p.t("Grid") }, { id: "list", label: p.t("List") }] }
        Item {
            width: parent.width; height: 48
            Text { anchors.verticalCenter: parent.verticalCenter; text: p.t("Show hidden files"); color: Theme.ink; font.pixelSize: 14 }
            Switch {
                id: hidden
                anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                checked: !!root.prefs.showHidden
                Accessible.name: p.t("Show hidden files")
                // Any change (a click, or an accessibility Toggle, which on Qt 6.4 does
                // not emit toggled()); writes only when it differs from the setting.
                onCheckedChanged: if (checked !== !!root.prefs.showHidden) p.setpref("show-hidden", checked ? "true" : "false")
            }
        }

        // Use as the default file manager (P6-T07).
        Item {
            width: parent.width; height: 56
            Column {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - 70
                Text { text: p.t("Use as default file manager"); color: Theme.ink; font.pixelSize: 14 }
                Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.muted; font.pixelSize: 12
                       text: p.t("Folders open in Cairn, and other apps' “Show in folder” comes here.") }
            }
            Switch {
                anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                checked: !!root.prefs.defaultFileManager
                Accessible.name: p.t("Use as default file manager")
                // Any change (a click, or an accessibility Toggle, which on Qt 6.4 does
                // not emit toggled()); writes only when it differs from the setting.
                onCheckedChanged: if (checked !== !!root.prefs.defaultFileManager) p.setpref("default-file-manager", checked ? "true" : "false")
            }
        }

        // Bookmarks (P7-T08): the app's only outbound request, disclosed here.
        Item {
            width: parent.width; height: 70
            Column {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - 70
                Text { text: p.t("Fetch link titles and pictures"); color: Theme.ink; font.pixelSize: 14 }
                Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.muted; font.pixelSize: 12
                       text: p.t("When you save a bookmark, Cairn contacts that link’s site for its title and picture. It is the only request Cairn makes.") }
            }
            Switch {
                anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                checked: !!root.prefs.bookmarkFetch
                Accessible.name: p.t("Fetch link titles and pictures")
                onCheckedChanged: if (checked !== !!root.prefs.bookmarkFetch) p.setpref("bookmark-fetch", checked ? "true" : "false")
            }
        }

        // Open In (P7-T02): which terminal (F4) and editor (Shift+F4).
        OpenInChoice { role: "terminal"; label: p.t("Terminal (F4)") }
        OpenInChoice { role: "editor"; label: p.t("Editor (Shift+F4)") }

        // Which folder is Downloads (P4-T06).
        Item {
            width: parent.width; height: 48
            Text { anchors.verticalCenter: parent.verticalCenter; text: p.t("Downloads folder"); color: Theme.ink; font.pixelSize: 14 }
            TextField {
                id: downloadsField
                anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                width: 230
                placeholderText: p.t("The system’s Downloads")
                text: root.prefs.downloadsFolder || ""
                Accessible.name: p.t("Downloads folder")
                color: Theme.ink
                background: Rectangle { radius: 9; color: Theme.pane; border.color: Theme.line }
                onAccepted: p.activate("win.set-downloads", text)
            }
        }

    }

    Column {
        id: appearance
        visible: root.page === "appearance"
        width: parent.width
        spacing: 2
        Choice { label: p.t("Style"); key: "color-scheme"
                 options: [{ id: "system", label: p.t("System") }, { id: "light", label: p.t("Light") }, { id: "dark", label: p.t("Dark") }] }
        // Colour: the system's accent, or one of Cairn's own (each with a
        // light and a dark shade, so it suits all three styles).
        Item {
            width: parent.width; height: 52
            readonly property bool followsSystem: (root.prefs.colorScheme || "system") === "system"
            Text { anchors.verticalCenter: parent.verticalCenter; text: p.t("Colour"); color: Theme.ink; font.pixelSize: 14 }
            // Style "System": the colour is Ubuntu's (Settings › Appearance).
            Row {
                visible: parent.followsSystem
                anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                spacing: 10
                Text { anchors.verticalCenter: parent.verticalCenter; color: Theme.muted; font.pixelSize: 12
                       text: p.t("Ubuntu’s accent colour, from Settings › Appearance")
                       Accessible.role: Accessible.StaticText; Accessible.name: text }
                Rectangle { width: 28; height: 28; radius: 14; color: Theme.systemAccentColor
                            anchors.verticalCenter: parent.verticalCenter }
            }
            // Light or Dark: one of Cairn's own colours.
            Row {
                visible: !parent.followsSystem
                anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                spacing: 8
                Accessible.role: Accessible.Grouping
                Accessible.name: p.t("Colour")
                Repeater {
                    model: [
                        { id: "cairn", label: p.t("Purple") },
                        { id: "blue", label: p.t("Blue") }, { id: "teal", label: p.t("Teal") },
                        { id: "green", label: p.t("Green") }, { id: "orange", label: p.t("Orange") },
                        { id: "rose", label: p.t("Rose") }, { id: "graphite", label: p.t("Graphite") }
                    ]
                    delegate: Rectangle {
                        id: swatch
                        required property var modelData
                        readonly property bool chosen: ((root.prefs.accent || "cairn") === "system" ? "cairn" : (root.prefs.accent || "cairn")) === modelData.id
                        readonly property var set: Theme.accents[modelData.id]
                        width: 28; height: 28; radius: 14
                        color: set[Theme.dark ? 1 : 0]
                        border.color: chosen ? Theme.ink : "transparent"
                        border.width: 2
                        Accessible.role: Accessible.RadioButton
                        Accessible.name: modelData.label
                        Accessible.checked: chosen
                        Accessible.onPressAction: p.setpref("accent", modelData.id)
                        Rectangle {
                            visible: swatch.chosen
                            anchors.centerIn: parent; width: 22; height: 22; radius: 11
                            color: "transparent"; border.color: Theme.pane; border.width: 2
                        }
                        ToolTip.visible: tipHover.hovered
                        ToolTip.text: modelData.label
                        HoverHandler { id: tipHover; cursorShape: Qt.PointingHandCursor }
                        TapHandler { onTapped: p.setpref("accent", swatch.modelData.id) }
                    }
                }
            }
        }
        Choice { label: p.t("Card size"); key: "row-size"
                 options: [{ id: "small", label: p.t("Small") }, { id: "medium", label: p.t("Medium") }, { id: "large", label: p.t("Large") }] }
        Text {
            width: parent.width; wrapMode: Text.WordWrap; color: Theme.muted; font.pixelSize: 13; topPadding: 10
            text: p.t("Animations follow the system: Settings › Accessibility › Seeing › Reduce Animation.")
        }
    }

    Column {
        id: shortcuts
        visible: root.page === "shortcuts"
        width: parent.width
        spacing: 2
        Item {
            width: parent.width; height: 44
            Text { anchors.verticalCenter: parent.verticalCenter; width: parent.width - reset.width - 12
                   wrapMode: Text.WordWrap; color: Theme.muted; font.pixelSize: 12
                   text: root.recording ? p.t("Press the new shortcut for “") + root.recordingName + p.t("”. Esc cancels.")
                                        : p.t("Choose Change, then press the keys you want.") }
            PillButton { id: reset; anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                         implicitHeight: 32; text: p.t("Restore Defaults")
                         onClicked: { root.recording = ""; p.activate("win.shortcuts-reset", "") } }
        }
        TextField {
            id: shortcutFilter
            width: parent.width
            placeholderText: p.t("Find a shortcut")
            Accessible.name: p.t("Find a shortcut")
            color: Theme.ink
            background: Rectangle { radius: 9; color: Theme.pane; border.color: shortcutFilter.activeFocus ? Theme.accent : Theme.line }
        }
        Item { width: 1; height: 6 }
        Item {
            width: parent.width
            height: list.implicitHeight
            Accessible.role: Accessible.List
            Accessible.name: p.t("Keyboard shortcuts")
            Column {
                id: list
                width: parent.width
                Repeater {
                    model: {
                        const q = shortcutFilter.text.trim().toLowerCase()
                        return q === "" ? root.accels
                            : root.accels.filter(a => a.description.toLowerCase().indexOf(q) >= 0
                                                 || a.keys.join(" ").toLowerCase().indexOf(q) >= 0)
                    }
                    delegate: Rectangle {
                        id: entry
                        required property var modelData
                        readonly property bool on: root.recording === modelData.id
                        width: shortcuts.width
                        height: 36
                        radius: 8
                        color: on ? Theme.alpha(Theme.accent, 0.12) : "transparent"
                        Accessible.role: Accessible.StaticText
                        Accessible.name: modelData.description + ": "
                                         + (modelData.keys.length ? modelData.keys.join(p.t(" or ")) : p.t("no shortcut"))
                        Text { x: 6; anchors.verticalCenter: parent.verticalCenter; text: entry.modelData.description
                               color: Theme.ink; font.pixelSize: 13; width: parent.width - right.width - 18; elide: Text.ElideRight
                               font.weight: entry.modelData.custom ? Font.DemiBold : Font.Normal }
                        Row {
                            id: right
                            anchors.right: parent.right; anchors.rightMargin: 4
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: 4
                            Text { visible: entry.on; text: p.t("Press keys…"); color: Theme.accentText; font.pixelSize: 12
                                   anchors.verticalCenter: parent.verticalCenter }
                            Text { visible: !entry.on && entry.modelData.keys.length === 0; text: p.t("None")
                                   color: Theme.faint; font.pixelSize: 12; anchors.verticalCenter: parent.verticalCenter }
                            Repeater {
                                model: entry.on || entry.modelData.keys.length === 0 ? []
                                       : entry.modelData.keys[0].split(/\+(?!$)/).filter(k => k !== "")
                                delegate: Rectangle {
                                    required property string modelData
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: Math.max(26, key.implicitWidth + 14); height: 24; radius: 6
                                    color: Theme.track; border.color: Theme.line
                                    Text { id: key; anchors.centerIn: parent; text: parent.modelData; color: Theme.ink; font.pixelSize: 12 }
                                }
                            }
                            IconButton { width: 26; height: 26; iconSize: 13; flat: true; icon: "pencil"
                                         label: p.t("Change shortcut for ") + entry.modelData.description
                                         onClicked: { root.recording = entry.modelData.id; root.recordingName = entry.modelData.description
                                                      recorder.forceActiveFocus() } }
                            IconButton { width: 26; height: 26; iconSize: 13; flat: true; icon: "close"
                                         enabled: entry.modelData.keys.length > 0
                                         label: p.t("Clear shortcut for ") + entry.modelData.description
                                         onClicked: p.activate("win.shortcut-clear", entry.modelData.id) }
                        }
                    }
                }
            }
        }
    }

    // Shelf and tray (P8-T02, P8-T03).
    Column {
        id: shelfPage
        visible: root.page === "shelf"
        width: parent.width
        spacing: 2
        Item {
            width: parent.width; height: 70
            Column {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - 70
                Text { text: p.t("Open the Shelf with a key"); color: Theme.ink; font.pixelSize: 14 }
                Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.muted; font.pixelSize: 12
                       text: p.t("Works from any app. Cairn adds it to GNOME’s keyboard shortcuts and removes it when you turn this off.") }
            }
            Switch {
                anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                checked: !!root.prefs.shelfShortcutEnabled
                Accessible.name: p.t("Open the Shelf with a key")
                onCheckedChanged: if (checked !== !!root.prefs.shelfShortcutEnabled) p.setpref("shelf-shortcut-enabled", checked ? "true" : "false")
            }
        }
        Item {
            width: parent.width; height: 44
            Text { anchors.verticalCenter: parent.verticalCenter; text: p.t("Key"); color: Theme.ink; font.pixelSize: 14 }
            Row {
                anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                spacing: 4
                Text { visible: root.recording === "__shelf"; text: p.t("Press keys…"); color: Theme.accentText
                       font.pixelSize: 12; anchors.verticalCenter: parent.verticalCenter }
                Repeater {
                    model: root.recording === "__shelf" ? [] : (root.prefs.shelfShortcut || "").split(/\+(?!$)/).filter(k => k !== "")
                    delegate: Rectangle {
                        required property string modelData
                        anchors.verticalCenter: parent.verticalCenter
                        width: Math.max(26, keyText.implicitWidth + 14); height: 24; radius: 6
                        color: Theme.track; border.color: Theme.line
                        Text { id: keyText; anchors.centerIn: parent; text: parent.modelData; color: Theme.ink; font.pixelSize: 12 }
                    }
                }
                IconButton { width: 26; height: 26; iconSize: 13; flat: true; icon: "pencil"
                             label: p.t("Change the Shelf key")
                             onClicked: { root.recording = "__shelf"; root.recordingName = p.t("Shelf"); recorder.forceActiveFocus() } }
            }
        }
        PrefSwitch {
            label: p.t("Show an icon in the top bar")
            note: p.t("Click it to open the Shelf. With the icon on, closing the window keeps Cairn running for the Shelf.")
            key: "tray-icon"; on: !!root.prefs.trayIcon
        }
        PrefSwitch {
            label: p.t("Open at login")
            note: p.t("Starts Cairn in the background, without a window, so the Shelf and the icon are ready.")
            key: "open-at-login"; on: !!root.prefs.openAtLogin
        }
    }

    Column {
        id: search
        visible: root.page === "search"
        width: parent.width
        spacing: 2
        // Activities search (P8-T04): what GNOME's search may show from Cairn.
        PrefSwitch { label: p.t("Workspaces in Activities search"); key: "search-provider-workspaces"
                     note: p.t("Type a workspace’s name in the Activities overview to open it.")
                     on: !!root.prefs.providerWorkspaces }
        PrefSwitch { label: p.t("Bookmark collections in Activities search"); key: "search-provider-collections"
                     note: p.t("Collections of saved links, by name.")
                     on: !!root.prefs.providerCollections }
        PrefSwitch { label: p.t("Links in Activities search"); key: "search-provider-links"
                     note: p.t("Saved links, by title, address or tag; they open in the browser.")
                     on: !!root.prefs.providerLinks }
        // Search and index (P3-T12).
        Text {
            width: parent.width
            wrapMode: Text.WordWrap
            color: Theme.muted; font.pixelSize: 13
            text: !root.prefs.indexRunning ? p.t("The index is not running.")
                  : root.prefs.indexPaused ? p.t("Indexing is paused.") : p.t("The index is up to date and following changes.")
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }
        Item {
            width: parent.width; height: 44
            Text { anchors.verticalCenter: parent.verticalCenter; text: p.t("Pause indexing"); color: Theme.ink; font.pixelSize: 14 }
            Switch {
                anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                checked: !!root.prefs.indexPaused
                Accessible.name: p.t("Pause indexing")
                // Any change (a click, or an accessibility Toggle, which on Qt 6.4 does
                // not emit toggled()); writes only when it differs from the setting.
                onCheckedChanged: if (checked !== !!root.prefs.indexPaused) p.activate("win.index-paused", checked ? "true" : "false")
            }
        }
        Item {
            width: parent.width; height: 44
            Text { anchors.verticalCenter: parent.verticalCenter; text: p.t("Pause on battery"); color: Theme.ink; font.pixelSize: 14 }
            Switch {
                anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                checked: !!root.prefs.indexPauseOnBattery
                Accessible.name: p.t("Pause on battery")
                // Any change (a click, or an accessibility Toggle, which on Qt 6.4 does
                // not emit toggled()); writes only when it differs from the setting.
                onCheckedChanged: if (checked !== !!root.prefs.indexPauseOnBattery) p.setpref("index-pause-on-battery", checked ? "true" : "false")
            }
        }
        Text {
            width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: 13; topPadding: 6
            text: p.t("Indexed: ") + ((root.prefs.indexRoots || []).length ? root.prefs.indexRoots.join(", ") : (root.prefs.home || p.t("Home")))
                  + ". Removable drives and network shares are left out."
        }
        Text { text: p.t("Left out"); color: Theme.ink; font.pixelSize: 14; topPadding: 8; visible: (root.prefs.indexExclude || []).length > 0 }
        Repeater {
            model: root.prefs.indexExclude || []
            delegate: Item {
                required property string modelData
                width: parent.width; height: 34
                Text { anchors.verticalCenter: parent.verticalCenter; text: parent.modelData; color: Theme.ink
                       font.pixelSize: 13; elide: Text.ElideMiddle; width: parent.width - 40 }
                IconButton { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                             width: 26; height: 26; iconSize: 12; flat: true; icon: "close"
                             label: p.t("Stop leaving out ") + parent.modelData
                             onClicked: p.activate("win.index-exclude", "-" + parent.modelData) }
            }
        }
        Row {
            width: parent.width
            spacing: 8
            topPadding: 6
            TextField {
                id: exclude
                width: parent.width - add.width - rebuild.width - 16
                placeholderText: p.t("A folder to leave out, e.g. ~/Videos/Raw")
                Accessible.name: p.t("Folder to leave out")
                color: Theme.ink
                background: Rectangle { radius: 9; color: Theme.pane; border.color: Theme.line }
                onAccepted: add.clicked()
            }
            PillButton { id: add; text: p.t("Leave Out")
                         onClicked: { if (exclude.text.trim() !== "") p.activate("win.index-exclude", "+" + exclude.text); exclude.text = "" } }
            PillButton { id: rebuild; text: p.t("Rebuild"); onClicked: p.activate("win.index-rebuild", "") }
        }
        }
        }
    }
}
