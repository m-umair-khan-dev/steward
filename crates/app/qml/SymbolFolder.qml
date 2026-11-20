import QtQuick

// A small filled folder with a white symbol on its front (Smart Clean cards).
Item {
    id: root
    property string kind: "folders"
    property string symbol: "folder"
    property int size: 56
    width: size
    height: size * 0.82
    MiniFolder { size: root.size; kind: root.kind }
    Icon {
        anchors.horizontalCenter: parent.horizontalCenter
        y: parent.height * 0.42
        name: root.symbol
        size: Math.round(root.size * 0.34)
        color: "white"
    }
}
