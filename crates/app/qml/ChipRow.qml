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
            Accessible.name: "Filter by kind"
            Repeater {
                model: root.chips
                delegate: Rectangle {
                    required property var modelData
                    width: label.implicitWidth + 26
                    height: 30
                    radius: 15
                    color: modelData.active ? Theme.alpha(Theme.accent, 0.16) : (hover.hovered ? Theme.hover : Theme.track)
                    Behavior on color { ColorAnimation { duration: 120 } }
                    Accessible.role: Accessible.Button
                    Accessible.name: label.text
                    Accessible.checked: modelData.active
                    Accessible.onPressAction: p.activate("win.filter-kind", modelData.id)
                    Text {
                        id: label
                        anchors.centerIn: parent
                    text: parent.modelData.label + " " + parent.modelData.count
                    color: parent.modelData.active ? Theme.accentText : Theme.ink
                    font.pixelSize: 13
                    font.weight: parent.modelData.active ? Font.Bold : Font.Medium
                }
                HoverHandler { id: hover }
                TapHandler { onTapped: p.activate("win.filter-kind", parent.modelData.id) }
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
