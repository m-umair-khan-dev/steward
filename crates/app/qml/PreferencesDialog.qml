import QtQuick
import QtQuick.Controls.Basic

// Preferences (P6-T03): General, Appearance, Shortcuts (the list of every
// key, from the same table that binds them) and Search and index.
Sheet {
    id: root
    property var prefs: ({})
    property var accels: []
    property string page: "general"
    title: "Preferences"
    cardWidth: 560
    cardHeight: 600
    onVisibleChanged: if (visible) page = "general"

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

    Segmented {
        id: tabs
        name: "Preferences pages"
        options: [{ id: "general", label: "General" }, { id: "appearance", label: "Appearance" },
                  { id: "shortcuts", label: "Shortcuts" }, { id: "search", label: "Search and Index" }]
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

    Column {
        id: general
        visible: root.page === "general"
        width: parent.width
        spacing: 2
        Choice { label: "Default view"; key: "default-view"
                 options: [{ id: "grid", label: "Grid" }, { id: "list", label: "List" }] }
        Item {
            width: parent.width; height: 48
            Text { anchors.verticalCenter: parent.verticalCenter; text: "Show hidden files"; color: Theme.ink; font.pixelSize: 14 }
            Switch {
                id: hidden
                anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                checked: !!root.prefs.showHidden
                Accessible.name: "Show hidden files"
                onToggled: p.setpref("show-hidden", checked ? "true" : "false")
            }
        }

        // Which folder is Downloads (P4-T06).
        Item {
            width: parent.width; height: 48
            Text { anchors.verticalCenter: parent.verticalCenter; text: "Downloads folder"; color: Theme.ink; font.pixelSize: 14 }
            TextField {
                id: downloadsField
                anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                width: 230
                placeholderText: "The system’s Downloads"
                text: root.prefs.downloadsFolder || ""
                Accessible.name: "Downloads folder"
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
        Choice { label: "Style"; key: "color-scheme"
                 options: [{ id: "system", label: "System" }, { id: "light", label: "Light" }, { id: "dark", label: "Dark" }] }
        Choice { label: "Accent"; key: "accent"
                 options: [{ id: "cairn", label: "Cairn" }, { id: "system", label: "System" }] }
        Choice { label: "Card size"; key: "row-size"
                 options: [{ id: "small", label: "Small" }, { id: "medium", label: "Medium" }, { id: "large", label: "Large" }] }
        Text {
            width: parent.width; wrapMode: Text.WordWrap; color: Theme.muted; font.pixelSize: 13; topPadding: 10
            text: "Animations follow the system: Settings › Accessibility › Seeing › Reduce Animation."
        }
    }

    Column {
        id: shortcuts
        visible: root.page === "shortcuts"
        width: parent.width
        Accessible.role: Accessible.List
        Accessible.name: "Keyboard shortcuts"
        Repeater {
            model: root.accels
            delegate: Item {
                required property var modelData
                width: shortcuts.width
                height: 34
                Accessible.role: Accessible.StaticText
                Accessible.name: modelData.description + ": " + modelData.keys[0]
                Text { anchors.verticalCenter: parent.verticalCenter; text: parent.modelData.description
                       color: Theme.ink; font.pixelSize: 13; width: parent.width - keys.width - 12; elide: Text.ElideRight }
                Row {
                    id: keys
                    anchors.right: parent.right; anchors.rightMargin: 8
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 4
                    Repeater {
                        model: parent.parent.modelData.keys[0].split("+").filter(k => k !== "")
                        delegate: Rectangle {
                            required property string modelData
                            width: Math.max(26, key.implicitWidth + 14); height: 24; radius: 6
                            color: Theme.track; border.color: Theme.line
                            Text { id: key; anchors.centerIn: parent; text: parent.modelData; color: Theme.ink; font.pixelSize: 12 }
                        }
                    }
                }
            }
        }
    }

    Column {
        id: search
        visible: root.page === "search"
        width: parent.width
        spacing: 2
        // Search and index (P3-T12).
        Text {
            width: parent.width
            wrapMode: Text.WordWrap
            color: Theme.muted; font.pixelSize: 13
            text: !root.prefs.indexRunning ? "The index is not running."
                  : root.prefs.indexPaused ? "Indexing is paused." : "The index is up to date and following changes."
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }
        Item {
            width: parent.width; height: 44
            Text { anchors.verticalCenter: parent.verticalCenter; text: "Pause indexing"; color: Theme.ink; font.pixelSize: 14 }
            Switch {
                anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                checked: !!root.prefs.indexPaused
                Accessible.name: "Pause indexing"
                onToggled: p.activate("win.index-paused", checked ? "true" : "false")
            }
        }
        Item {
            width: parent.width; height: 44
            Text { anchors.verticalCenter: parent.verticalCenter; text: "Pause on battery"; color: Theme.ink; font.pixelSize: 14 }
            Switch {
                anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                checked: !!root.prefs.indexPauseOnBattery
                Accessible.name: "Pause on battery"
                onToggled: p.setpref("index-pause-on-battery", checked ? "true" : "false")
            }
        }
        Text {
            width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: 13; topPadding: 6
            text: "Indexed: " + ((root.prefs.indexRoots || []).length ? root.prefs.indexRoots.join(", ") : (root.prefs.home || "Home"))
                  + ". Removable drives and network shares are left out."
        }
        Text { text: "Left out"; color: Theme.ink; font.pixelSize: 14; topPadding: 8; visible: (root.prefs.indexExclude || []).length > 0 }
        Repeater {
            model: root.prefs.indexExclude || []
            delegate: Item {
                required property string modelData
                width: parent.width; height: 34
                Text { anchors.verticalCenter: parent.verticalCenter; text: parent.modelData; color: Theme.ink
                       font.pixelSize: 13; elide: Text.ElideMiddle; width: parent.width - 40 }
                IconButton { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                             width: 26; height: 26; iconSize: 12; flat: true; icon: "close"
                             label: "Stop leaving out " + parent.modelData
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
                placeholderText: "A folder to leave out, e.g. ~/Videos/Raw"
                Accessible.name: "Folder to leave out"
                color: Theme.ink
                background: Rectangle { radius: 9; color: Theme.pane; border.color: Theme.line }
                onAccepted: add.clicked()
            }
            PillButton { id: add; text: "Leave Out"
                         onClicked: { if (exclude.text.trim() !== "") p.activate("win.index-exclude", "+" + exclude.text); exclude.text = "" } }
            PillButton { id: rebuild; text: "Rebuild"; onClicked: p.activate("win.index-rebuild", "") }
        }
        }
        }
    }
}
