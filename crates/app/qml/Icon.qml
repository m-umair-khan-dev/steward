import QtQuick

// One of our own line icons (qml/icons), recoloured. With QtQuick.Effects
// (Qt 6.5+) the GPU tints it; on Qt 6.4 the SVG itself is recoloured
// (p.svgicon), which looks the same.
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
        source: !root.name ? "" : (Theme.effects ? "qrc:/icons/" + root.name + ".svg" : p.svgicon(root.name, root.color.toString()))
        sourceSize: Qt.size(root.size * 2, root.size * 2)
        visible: !Theme.effects
        smooth: true
    }
    Loader {
        anchors.fill: parent
        active: Theme.effects
        source: "qrc:/fx/Tint.qml"
        onLoaded: {
            item.source = image
            item.colorizationColor = Qt.binding(() => root.color)
        }
    }
}
