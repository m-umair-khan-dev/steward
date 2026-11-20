import QtQuick
import QtQuick.Controls.Basic

// New workspace, or edit one: name, description, colour.
Sheet {
    id: root
    property var info: ({ colors: [] })
    property bool editing: false
    property string color: info.color || ((info.colors || [])[0] || "folders")
    title: editing ? "Edit Workspace" : "New Workspace"
    cardWidth: 440
    onVisibleChanged: if (visible) { name.text = info.name || ""; description.text = info.description || ""
                                     root.color = info.color || ((info.colors || [])[0] || "folders"); name.forceActiveFocus() }

    function save() {
        if (name.text.trim() === "") return
        if (editing) p.activate("win.workspace-edit", info.id + "|" + name.text + "|" + description.text + "|" + root.color)
        else p.activate("win.workspace-create", name.text)
    }

    Column {
        width: parent.width
        spacing: 12
        TextField {
            id: name
            width: parent.width
            placeholderText: "Name, e.g. Taxes 2026"
            Accessible.name: "Workspace name"
            color: Theme.ink
            background: Rectangle { radius: 9; color: Theme.pane; border.color: name.activeFocus ? Theme.accent : Theme.line }
            onAccepted: root.save()
        }
        TextField {
            id: description
            visible: root.editing
            width: parent.width
            placeholderText: "What it is for"
            Accessible.name: "Workspace description"
            color: Theme.ink
            background: Rectangle { radius: 9; color: Theme.pane; border.color: Theme.line }
            onAccepted: root.save()
        }
        Row {
            visible: root.editing
            spacing: 8
            Repeater {
                model: root.info.colors || []
                delegate: Rectangle {
                    required property string modelData
                    width: 28; height: 28; radius: 14
                    color: Theme.deep(modelData)
                    border.color: root.color === modelData ? Theme.ink : "transparent"
                    border.width: 2
                    Accessible.role: Accessible.Button
                    Accessible.name: "Colour " + modelData
                    MouseArea { anchors.fill: parent; onClicked: root.color = parent.modelData }
                }
            }
        }
        Row {
            anchors.right: parent.right
            spacing: 8
            PillButton { visible: root.editing; text: "Delete"; destructive: true
                         onClicked: p.activate("win.workspace-delete", String(root.info.id)) }
            PillButton { text: "Cancel"; onClicked: root.close() }
            PillButton { text: root.editing ? "Save" : "Create"; primary: true; onClicked: root.save() }
        }
    }
}
