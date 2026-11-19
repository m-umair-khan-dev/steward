import QtQuick
import QtQuick.Controls.Basic

// The right pane (DESIGN.md §3.3): folder overview with space by kind when
// nothing is selected, the item's preview and facts when one item is.
Rectangle {
    id: root
    property var model: ({ mode: "empty" })
    color: Theme.paneSide
    Accessible.role: Accessible.Pane
    Accessible.name: "Preview"

    Rectangle { width: 1; height: parent.height; color: Theme.line }

    Flickable {
        anchors.fill: parent
        anchors.margins: 20
        contentHeight: content.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds

        Column {
            id: content
            width: parent.width
            spacing: 14

            // --- overview -----------------------------------------------------
            Column {
                visible: root.model.mode === "overview"
                width: parent.width
                spacing: 6
                Text { text: root.model.title || ""; color: Theme.ink; font.pixelSize: 18; font.weight: Font.Bold
                       width: parent.width; elide: Text.ElideRight
                       Accessible.role: Accessible.Heading; Accessible.name: text }
                Row {
                    spacing: 8
                    Text { id: total; text: root.model.total || ""; color: Theme.ink; font.pixelSize: 30; font.weight: Font.Bold }
                    Text { text: "in all"; color: Theme.muted; font.pixelSize: 16
                           anchors.baseline: total.baseline }
                }
                Text { text: root.model.counts || ""; color: Theme.muted; font.pixelSize: 13 }
                Item { width: 1; height: 10 }
                Text { text: "WHAT TAKES THE SPACE"; color: Theme.faint; font.pixelSize: 12; font.weight: Font.DemiBold
                       font.letterSpacing: 0.6
                       visible: (root.model.kinds || []).length > 0 }
                Repeater {
                    model: root.model.kinds || []
                    delegate: Column {
                        required property var modelData
                        required property int index
                        width: parent.width
                        spacing: 5
                        topPadding: 6
                        Item {
                            width: parent.width; height: 18
                            Rectangle { width: 10; height: 10; radius: 3; color: Theme.deep(parent.parent.modelData.kind)
                                        anchors.verticalCenter: parent.verticalCenter }
                            Text { x: 18; text: parent.parent.modelData.label; color: Theme.ink; font.pixelSize: 13
                                   anchors.verticalCenter: parent.verticalCenter }
                            Text { anchors.right: parent.right; text: parent.parent.modelData.size; color: Theme.muted
                                   font.pixelSize: 13; anchors.verticalCenter: parent.verticalCenter }
                        }
                        Rectangle {
                            width: parent.width; height: 6; radius: 3; color: Theme.track
                            Rectangle {
                                height: parent.height; radius: 3
                                color: Theme.deep(parent.parent.modelData.kind)
                                width: parent.width * parent.parent.modelData.frac * grow.value
                            }
                        }
                        // The bars grow in when the overview appears.
                        NumberAnimation on opacity { from: 0; to: 1; duration: 200 }
                        QtObject { id: grow; property real value: 0 }
                        NumberAnimation { target: grow; property: "value"; from: 0; to: 1; duration: 420
                                          easing.type: Easing.OutCubic; running: true }
                    }
                }
                Text { visible: (root.model.more || 0) > 0; text: "and " + root.model.more + " more kinds"
                       color: Theme.faint; font.pixelSize: 12; topPadding: 4 }
            }

            // --- one item -----------------------------------------------------
            Column {
                visible: root.model.mode === "item"
                width: parent.width
                spacing: 10
                Rectangle {
                    width: parent.width
                    height: width * 0.72
                    radius: 14
                    color: Theme.sunken
                    clip: true
                    Image {
                        id: thumb
                        anchors.fill: parent
                        source: root.model.thumb ? "file://" + root.model.thumb : ""
                        fillMode: Image.PreserveAspectFit
                        asynchronous: true
                        sourceSize.width: width * 2
                    }
                    GlassFolder {
                        visible: !!root.model.dir
                        anchors.centerIn: parent
                        width: parent.width * 0.6
                        kind: "folders"
                        count: root.model.childCount || ""
                        enabled: false
                    }
                    Rectangle {
                        visible: !root.model.dir && thumb.status !== Image.Ready
                        anchors.centerIn: parent
                        width: parent.width * 0.5; height: parent.height * 0.6; radius: 10
                        color: Theme.deep(root.model.kind || "other")
                        Text { anchors.centerIn: parent; text: root.model.badge || ""; color: "white"
                               font.pixelSize: 18; font.weight: Font.Black }
                    }
                }
                Text {
                    width: parent.width
                    text: root.model.name || ""
                    color: Theme.ink
                    font.pixelSize: 16; font.weight: Font.Bold
                    wrapMode: Text.WrapAnywhere
                    maximumLineCount: 3
                }
                Repeater {
                    model: root.model.facts || []
                    delegate: Item {
                        required property var modelData
                        width: parent.width
                        height: Math.max(key.height, value.height) + 4
                        Text { id: key; text: parent.modelData[0]; color: Theme.faint; font.pixelSize: 12; width: 76 }
                        Text { id: value; x: 80; width: parent.width - 80; text: parent.modelData[1]; color: Theme.ink
                               font.pixelSize: 12; wrapMode: Text.WordWrap
                               Accessible.role: Accessible.StaticText; Accessible.name: parent.modelData[0] + " " + text }
                    }
                }
                // The index's outline, two levels deep (reference: "a folder at a glance").
                Column {
                    visible: !!root.model.dir && !!root.model.tree
                    width: parent.width
                    spacing: 2
                    topPadding: 6
                    Text { text: "CONTENTS"; color: Theme.faint; font.pixelSize: 12; font.weight: Font.DemiBold
                           font.letterSpacing: 0.6; bottomPadding: 4 }
                    Repeater {
                        model: root.model.tree || []
                        delegate: Item {
                            required property var modelData
                            width: parent.width
                            height: 26
                            Icon {
                                x: 4 + parent.modelData.depth * 18
                                anchors.verticalCenter: parent.verticalCenter
                                name: parent.modelData.dir ? "folder" : "document"
                                size: 15
                                color: parent.modelData.dir ? Theme.deep("folders") : Theme.faint
                            }
                            Text {
                                x: 26 + parent.modelData.depth * 18
                                width: parent.width - x - count.implicitWidth - 8
                                anchors.verticalCenter: parent.verticalCenter
                                text: parent.modelData.name
                                elide: Text.ElideMiddle
                                color: Theme.ink
                                font.pixelSize: 13
                            }
                            Text {
                                id: count
                                anchors.right: parent.right
                                anchors.verticalCenter: parent.verticalCenter
                                text: parent.modelData.items
                                color: Theme.faint
                                font.pixelSize: 12
                            }
                        }
                    }
                }
                Column {
                    visible: !!root.model.dir && !root.model.tree && (root.model.children || []).length > 0
                    width: parent.width
                    spacing: 4
                    topPadding: 6
                    Text { text: "CONTENTS"; color: Theme.faint; font.pixelSize: 12; font.weight: Font.DemiBold
                           font.letterSpacing: 0.6 }
                    Repeater {
                        model: root.model.children || []
                        delegate: Text { required property string modelData; text: modelData; color: Theme.ink
                                         font.pixelSize: 12; width: parent.width; elide: Text.ElideMiddle }
                    }
                    Text { visible: (root.model.more || 0) > 0; text: "and " + root.model.more + " more"
                           color: Theme.faint; font.pixelSize: 12 }
                }
            }

            Text {
                visible: root.model.mode === "empty"
                text: "Nothing to preview"
                color: Theme.faint
                font.pixelSize: 13
            }
        }
    }
}
