import QtQuick

// Makes its parent card draggable to other folders, the sidebar and other
// apps (P2-T05). Carries text/uri-list; the drop decides move or copy.
Item {
    id: proxy
    property string uri: ""
    property string uris: ""
    property alias area: mouse
    signal clicked(var mouse)
    signal doubleClicked(var mouse)

    Drag.active: mouse.drag.active
    Drag.dragType: Drag.Automatic
    Drag.supportedActions: Qt.MoveAction | Qt.CopyAction
    Drag.proposedAction: Qt.MoveAction
    Drag.mimeData: ({ "text/uri-list": proxy.uris })

    MouseArea {
        id: mouse
        parent: proxy.parent
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        drag.target: proxy
        drag.threshold: 8
        onPressed: proxy.uris = p.dragged(proxy.uri)
        onReleased: { proxy.x = 0; proxy.y = 0 }
        onClicked: mouse => proxy.clicked(mouse)
        onDoubleClicked: mouse => proxy.doubleClicked(mouse)
    }
}
