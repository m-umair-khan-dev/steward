import QtQuick
import QtQuick.Controls.Basic

// A menu in our style. model: [{ text, action, param, checked } | { separator: true }].
Menu {
    id: root
    property var model: []

    padding: 6
    background: Rectangle {
        implicitWidth: 220
        radius: 12
        color: Theme.card
        border.color: Theme.line
        layer.enabled: Theme.effects
        layer.samplerName: "tex"
        layer.effect: FxShadow { shadowOpacity: 0.22; shadowBlur: 0.8; shadowVerticalOffset: 6 }
    }

    Instantiator {
        model: root.model
        delegate: MenuItem {
            id: item
            required property var modelData
            text: modelData.text || ""
            enabled: !modelData.separator && modelData.enabled !== false
            height: modelData.separator ? 9 : 32
            onTriggered: p.activate(modelData.action, modelData.param || "")
            contentItem: Item {
                Rectangle {
                    visible: !!item.modelData.separator
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width; height: 1; color: Theme.line
                }
                Icon {
                    visible: !!item.modelData.checked
                    anchors.verticalCenter: parent.verticalCenter
                    name: "check"; size: 15; color: Theme.accentText
                }
                Text {
                    visible: !item.modelData.separator
                    x: 24
                    anchors.verticalCenter: parent.verticalCenter
                    text: item.text
                    color: item.enabled ? Theme.ink : Theme.faint
                    font.pixelSize: 13
                }
            }
            background: Rectangle {
                radius: 8
                color: item.highlighted && !item.modelData.separator ? Theme.hover : "transparent"
            }
        }
        onObjectAdded: (index, object) => root.insertItem(index, object)
        onObjectRemoved: (index, object) => root.removeItem(object)
    }
}
