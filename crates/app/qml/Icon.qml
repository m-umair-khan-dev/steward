import QtQuick
import QtQuick.Effects

// One of our own line icons (qml/icons), recoloured.
Item {
    id: root
    property string name: "folder"
    property color color: Theme.ink
    property int size: 18

    implicitWidth: size
    implicitHeight: size

    Image {
        id: image
        anchors.fill: parent
        source: root.name ? "qrc:/icons/" + root.name + ".svg" : ""
        sourceSize: Qt.size(root.size * 2, root.size * 2)
        visible: false
        smooth: true
    }
    MultiEffect {
        anchors.fill: parent
        source: image
        brightness: 1.0
        colorization: 1.0
        colorizationColor: root.color
    }
}
