import QtQuick

// Kind chips with counts and the folder summary (DESIGN.md §3.3).
Item {
    id: root
    property var chips: []
    property string summary: ""
    height: chips.length ? 50 : 14

    // Chips scroll sideways when they do not fit next to the summary.
    Flickable {
        x: 22
        width: Math.max(0, parent.width - x - summaryText.implicitWidth - 44)
        height: 32
        anchors.verticalCenter: parent.verticalCenter
        contentWidth: chipRow.implicitWidth
        clip: true
        flickableDirection: Flickable.HorizontalFlick
        boundsBehavior: Flickable.StopAtBounds
        Row {
            id: chipRow
            y: 1
            spacing: 6
            Accessible.role: Accessible.Grouping
            Accessible.name: p.t("Filter by kind")
            Repeater {
                model: root.chips
                // White with a border and a kind dot; the active chip is
                // lavender with an accent edge (reference: search, Installed).
                delegate: Rectangle {
                    id: chip
                    required property var modelData
                    readonly property bool dotted: modelData.id !== "all"
                    width: label.implicitWidth + (dotted ? 40 : 26)
                    height: 30
                    radius: 15
                    color: modelData.active ? Theme.alpha(Theme.accent, 0.12) : (hover.hovered ? Theme.sunken : Theme.pane)
                    border.color: modelData.active ? Theme.accent : Theme.line
                    border.width: 1
                    Behavior on color { ColorAnimation { duration: 120 } }
                    Accessible.role: Accessible.Button
                    Accessible.name: modelData.label + " " + modelData.count
                    Accessible.checked: modelData.active
                    Accessible.onPressAction: p.activate("win.filter-kind", modelData.id)
                    Rectangle {
                        visible: chip.dotted
                        x: 13; anchors.verticalCenter: parent.verticalCenter
                        width: 9; height: 9; radius: 2.5
                        color: Theme.deep(chip.modelData.id)
                    }
                    Text {
                        id: label
                        x: chip.dotted ? 28 : 13
                        anchors.verticalCenter: parent.verticalCenter
                        textFormat: Text.StyledText
                        text: chip.modelData.label + "&nbsp;&nbsp;<font color='" + Theme.faint + "'>" + chip.modelData.count + "</font>"
                        color: chip.modelData.active ? Theme.accentText : Theme.ink
                        font.pixelSize: 13
                        font.weight: chip.modelData.active ? Font.Bold : Font.Medium
                    }
                    HoverHandler { id: hover }
                    TapHandler { onTapped: p.activate("win.filter-kind", chip.modelData.id) }
            }
        }
    }
    }
    Text {
        id: summaryText
        anchors.right: parent.right; anchors.rightMargin: 22
        anchors.verticalCenter: parent.verticalCenter
        text: root.summary
        color: Theme.muted
        font.pixelSize: 13
    }
}
