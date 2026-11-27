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
    Accessible.name: p.t("Location")

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
        Accessible.name: p.t("Location")
        background: Rectangle { radius: 8; color: Theme.track; border.color: Theme.accent; border.width: 1.5 }
        Keys.onTabPressed: (event) => { text = p.complete(text); cursorPosition = text.length; event.accepted = true }
        Keys.onDownPressed: p.activate("win.path-match", "down")
        Keys.onUpPressed: p.activate("win.path-match", "up")
        Keys.onEscapePressed: p.cancelpath()
        onAccepted: p.submit(text)
        onTextChanged: if (visible) p.typed(text)
        onActiveFocusChanged: if (!activeFocus && visible) p.cancelpath()
    }

    // The Git chip (P7-T01): the repository's branch and its uncommitted changes.
    Rectangle {
        id: gitChip
        visible: !root.model.editing && !!root.model.git
        anchors.right: hint.left; anchors.rightMargin: 14
        anchors.verticalCenter: parent.verticalCenter
        height: 26; radius: 13
        width: chipRow.implicitWidth + 20
        color: Theme.alpha(Theme.accent, 0.12)
        border.color: Theme.alpha(Theme.accent, 0.35)
        Accessible.role: Accessible.StaticText
        Accessible.name: root.model.git
                         ? p.t("Git branch ") + root.model.git.branch + ", " + root.model.git.changes + p.t(" uncommitted changes")
                         : ""
        Row {
            id: chipRow
            anchors.centerIn: parent
            spacing: 6
            Icon { name: "branch"; size: 14; color: Theme.accentText; anchors.verticalCenter: parent.verticalCenter }
            Text { text: root.model.git ? root.model.git.branch : ""; color: Theme.accentText
                   font.pixelSize: 12; font.weight: Font.DemiBold; anchors.verticalCenter: parent.verticalCenter }
            Rectangle {
                visible: !!root.model.git && root.model.git.changes > 0
                width: Math.max(18, changesText.implicitWidth + 10); height: 18; radius: 9
                color: Theme.accentFill
                anchors.verticalCenter: parent.verticalCenter
                Text { id: changesText; anchors.centerIn: parent; text: root.model.git ? root.model.git.changes : ""
                       color: Theme.accentInk; font.pixelSize: 11; font.weight: Font.Bold }
            }
        }
    }

    Text {
        id: hint
        anchors.right: parent.right; anchors.rightMargin: 18
        anchors.verticalCenter: parent.verticalCenter
        visible: !root.model.editing
        text: p.t("Ctrl+L to type a path")
        color: Theme.faint
        font.pixelSize: 12
    }

    // Typing a folder's name lists every folder with that name and where it
    // is (P3-T10).
    Rectangle {
        visible: root.model.editing && (root.model.matches || []).length > 0
        x: 12
        y: parent.height + 4
        width: Math.min(560, parent.width - 24)
        height: matchList.implicitHeight + 12
        radius: 12
        color: Theme.card
        border.color: Theme.line
        z: 50
        Accessible.role: Accessible.List
        Accessible.name: p.t("Folders with that name")
        Column {
            id: matchList
            x: 6; y: 6
            width: parent.width - 12
            Repeater {
                model: root.model.matches || []
                delegate: Rectangle {
                    id: match
                    required property var modelData
                    width: matchList.width
                    height: 46
                    radius: 8
                    color: modelData.chosen ? Theme.alpha(Theme.accent, 0.14) : (matchHover.hovered ? Theme.hover : "transparent")
                    Accessible.role: Accessible.ListItem
                    Accessible.name: modelData.name + " " + modelData.where
                    Accessible.onPressAction: p.activate("win.path-match", String(modelData.index))
                    Icon { x: 10; anchors.verticalCenter: parent.verticalCenter; name: "folder"; size: 20
                           color: Theme.deep("folders") }
                    Column {
                        x: 40; anchors.verticalCenter: parent.verticalCenter
                        Text { text: match.modelData.name; color: Theme.ink; font.pixelSize: 14; font.weight: Font.DemiBold }
                        Text { text: match.modelData.where; color: Theme.muted; font.pixelSize: 12 }
                    }
                    HoverHandler { id: matchHover }
                    MouseArea { anchors.fill: parent; onClicked: p.activate("win.path-match", String(match.modelData.index)) }
                }
            }
        }
    }
}
