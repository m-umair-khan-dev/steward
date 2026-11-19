import QtQuick
import QtQuick.Controls.Basic

// The breadcrumb row (DESIGN.md §3.3): full path, current segment bold,
// "Ctrl+L to type a path" hint. Ctrl+L turns it into a field with Tab completion.
Rectangle {
    id: root
    property var model: ({ segments: [], editing: false, text: "" })
    property alias field: field
    height: 40
    color: Theme.pane
    Accessible.role: Accessible.Grouping
    Accessible.name: "Location"

    Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.line }

    Row {
        id: crumbs
        x: 18
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2
        visible: !root.model.editing
        Repeater {
            model: root.model.segments || []
            delegate: Row {
                required property var modelData
                required property int index
                readonly property bool last: index === (root.model.segments || []).length - 1
                spacing: 2
                Text {
                    visible: parent.index > 0
                    text: "›"
                    color: Theme.faint
                    font.pixelSize: 14
                    anchors.verticalCenter: parent.verticalCenter
                }
                Text {
                    text: parent.modelData.label
                    color: parent.last ? Theme.ink : Theme.muted
                    font.pixelSize: 14
                    font.weight: parent.last ? Font.Bold : Font.Normal
                    leftPadding: 4; rightPadding: 4
                    anchors.verticalCenter: parent.verticalCenter
                    Accessible.role: Accessible.Button
                    Accessible.name: text
                    Accessible.onPressAction: p.activate("win.go-to", parent.modelData.path)
                    TapHandler { onTapped: p.activate("win.go-to", parent.parent.modelData.path) }
                }
            }
        }
    }

    TextField {
        id: field
        x: 12
        width: parent.width - 24
        height: 30
        anchors.verticalCenter: parent.verticalCenter
        visible: root.model.editing
        color: Theme.ink
        font.pixelSize: 13
        Accessible.name: "Location"
        background: Rectangle { radius: 8; color: Theme.track; border.color: Theme.accent; border.width: 1.5 }
        Keys.onTabPressed: (event) => { text = p.complete(text); cursorPosition = text.length; event.accepted = true }
        Keys.onEscapePressed: p.cancelpath()
        onAccepted: p.submit(text)
        onTextChanged: if (visible) p.typed(text)
        onActiveFocusChanged: if (!activeFocus && visible) p.cancelpath()
    }

    Text {
        anchors.right: parent.right; anchors.rightMargin: 18
        anchors.verticalCenter: parent.verticalCenter
        visible: !root.model.editing
        text: "Ctrl+L to type a path"
        color: Theme.faint
        font.pixelSize: 12
    }
}
