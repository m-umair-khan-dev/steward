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
    }
}
