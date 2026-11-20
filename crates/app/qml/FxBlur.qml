import QtQuick

// The frosted-glass blur of a glass folder's front (Qt 6.5+). On Qt 6.4 it
// draws nothing and the front stays translucent.
Item {
    id: fx
    property Item source
    property Item mask

    Loader {
        anchors.fill: parent
        active: Theme.effects
        source: "qrc:/fx/Blur.qml"
        onLoaded: {
            item.source = Qt.binding(() => fx.source)
            item.maskSource = Qt.binding(() => fx.mask)
        }
    }
}
