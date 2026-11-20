import QtQuick
import QtQuick.Controls.Basic

// The queue (P2-T07): running and waiting jobs with progress and Cancel.
Popup {
    id: root
    property var jobs: []
    width: 340
    padding: 14
    background: Rectangle {
        radius: 14
        color: Theme.card
        border.color: Theme.line
        layer.enabled: Theme.effects
        layer.samplerName: "tex"
        layer.effect: FxShadow { shadowOpacity: 0.22; shadowBlur: 0.8; shadowVerticalOffset: 6 }
    }
    contentItem: Column {
        spacing: 12
        Accessible.role: Accessible.List
        Accessible.name: p.t("Operations")
        Text {
            visible: root.jobs.length === 0
            text: p.t("Nothing is running.")
            color: Theme.muted
            font.pixelSize: 13
        }
        Repeater {
            model: root.jobs
            delegate: Column {
                required property var modelData
                width: 312
                spacing: 6
                Item {
                    width: parent.width; height: 30
                    Text {
                        width: parent.width - 44
                        anchors.verticalCenter: parent.verticalCenter
                        text: parent.parent.modelData.summary
                        elide: Text.ElideMiddle
                        color: Theme.ink
                        font.pixelSize: 13
                        font.weight: Font.DemiBold
                    }
                    IconButton {
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        width: 28; height: 28; iconSize: 13
                        icon: "close"
                        label: p.t("Cancel ") + parent.parent.modelData.summary
                        onClicked: p.activate("win.cancel-job", parent.parent.modelData.id)
                    }
                }
                Rectangle {
                    width: parent.width; height: 6; radius: 3
                    color: Theme.track
                    Rectangle {
                        height: parent.height; radius: 3
                        color: Theme.accent
                        width: parent.width * Math.max(0.02, parent.parent.modelData.fraction)
                        Behavior on width { NumberAnimation { duration: 120 } }
                    }
                }
                Text {
                    text: parent.modelData.running ? (parent.modelData.progress || p.t("Working…")) : p.t("Waiting")
                    color: Theme.muted
                    font.pixelSize: 12
                }
            }
        }
    }
}
