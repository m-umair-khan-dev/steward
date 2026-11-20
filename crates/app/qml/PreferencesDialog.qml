import QtQuick
import QtQuick.Controls.Basic

// Preferences (DESIGN.md §3.3): view, size, style, accent, hidden files.
Sheet {
    id: root
    property var prefs: ({})
    title: "Preferences"
    cardWidth: 480

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

    Column {
        width: parent.width
        spacing: 2
        Choice { label: "Default view"; key: "default-view"
                 options: [{ id: "grid", label: "Grid" }, { id: "list", label: "List" }] }
        Choice { label: "Card size"; key: "row-size"
                 options: [{ id: "small", label: "Small" }, { id: "medium", label: "Medium" }, { id: "large", label: "Large" }] }
        Choice { label: "Style"; key: "color-scheme"
                 options: [{ id: "system", label: "System" }, { id: "light", label: "Light" }, { id: "dark", label: "Dark" }] }
        Choice { label: "Accent"; key: "accent"
                 options: [{ id: "cairn", label: "Cairn" }, { id: "system", label: "System" }] }
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

        // Search and index (P3-T12).
        Text { text: "SEARCH AND INDEX"; color: Theme.faint; font.pixelSize: 12; font.weight: Font.DemiBold
               font.letterSpacing: 0.6; topPadding: 14; bottomPadding: 4 }
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
