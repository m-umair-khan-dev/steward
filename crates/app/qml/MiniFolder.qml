import QtQuick

// A small filled folder in a kind's colours (sidebar workspaces, cards).
Item {
    id: root
    property string kind: "folders"
    property int size: 22
    width: size
    height: size * 0.82
    Rectangle { width: parent.width * 0.42; height: parent.height * 0.3; radius: 2.5; color: Theme.deep(root.kind) }
    Rectangle {
        y: parent.height * 0.14
        width: parent.width; height: parent.height * 0.86; radius: 3
        gradient: Gradient {
            GradientStop { position: 0; color: Theme.light(root.kind) }
            GradientStop { position: 1; color: Theme.deep(root.kind) }
        }
    }
    Rectangle { x: parent.width * 0.12; y: parent.height * 0.22; width: parent.width * 0.76; height: parent.height * 0.18
                radius: 1.5; color: Theme.onFill(0.85) }
}
