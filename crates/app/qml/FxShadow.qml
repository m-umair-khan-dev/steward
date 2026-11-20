import QtQuick

// A drop shadow for `layer.effect` that works on every Qt: with QtQuick.Effects
// (Qt 6.5+) it loads a MultiEffect; without it (Qt 6.4) the layer is off and
// nothing is drawn. Use with `layer.enabled: Theme.effects` and
// `layer.samplerName: "tex"`.
Item {
    id: fx
    property var tex
    property color shadowColor: "black"
    property real shadowOpacity: 0.3
    property real shadowBlur: 0.8
    property real shadowVerticalOffset: 4

    Loader {
        anchors.fill: parent
        active: Theme.effects
        source: "qrc:/fx/Shadow.qml"
        onLoaded: {
            item.source = Qt.binding(() => fx.tex)
            item.shadowColor = Qt.binding(() => fx.shadowColor)
            item.shadowOpacity = Qt.binding(() => fx.shadowOpacity)
            item.shadowBlur = Qt.binding(() => fx.shadowBlur)
            item.shadowVerticalOffset = Qt.binding(() => fx.shadowVerticalOffset)
        }
    }
}
