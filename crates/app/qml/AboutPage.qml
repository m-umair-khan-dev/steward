import QtQuick
import QtQuick.Controls.Basic

// About This Computer (P5-T11; reference: about-this-mac.gif): a picture of
// the machine, its name and summary, chips, then cards of facts in two
// columns. Hide Names and Copy as Text sit at the top right.
Rectangle {
    id: root
    property var model: ({ looking: true, sections: [] })
    color: Theme.pane
    Accessible.role: Accessible.Pane
    Accessible.name: p.t("About This Computer")

    Flickable {
        id: flick
        anchors.fill: parent
        contentHeight: page.implicitHeight + 40
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

        Column {
            id: page
            x: 32; y: 18
            width: flick.width - 64
            spacing: 14

            Row {
                anchors.right: parent.right
                spacing: 8
                PillButton {
                    text: root.model.hideNames ? p.t("Show Names") : p.t("Hide Names")
                    Accessible.name: p.t("Hide Names")
                    Accessible.checkable: true
                    Accessible.checked: !!root.model.hideNames
                    onClicked: p.activate("win.about-hide-names", root.model.hideNames ? "false" : "true")
                }
                PillButton { text: p.t("Copy as Text"); enabled: !root.model.looking; onClicked: p.activate("win.about-copy", "") }
            }

            // Our own drawing of a laptop (or a monitor for a desktop).
            Item {
                anchors.horizontalCenter: parent.horizontalCenter
                width: 190; height: 112
                Rectangle {
                    id: screen
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: root.model.laptop === false ? 150 : 150; height: 96; radius: 8
                    color: Theme.device[0]
                    Rectangle {
                        anchors.fill: parent; anchors.margins: 5; radius: 4
                        gradient: Gradient {
                            GradientStop { position: 0; color: Theme.light("folders") }
                            GradientStop { position: 1; color: Theme.deep("folders") }
                        }
                    }
                }
                Rectangle {
                    visible: root.model.laptop !== false
                    anchors.top: screen.bottom
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: 190; height: 10; radius: 4
                    gradient: Gradient {
                        GradientStop { position: 0; color: Theme.device[1] }
                        GradientStop { position: 1; color: Theme.device[2] }
                    }
                }
                Rectangle {
                    visible: root.model.laptop === false
                    anchors.top: screen.bottom
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: 18; height: 14; color: Theme.device[3]
                }
            }

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: root.model.looking ? p.t("Looking…") : (root.model.title || "")
                color: Theme.ink; font.pixelSize: 26; font.weight: Font.Bold
                Accessible.role: Accessible.Heading; Accessible.name: text
            }
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                width: Math.min(implicitWidth, parent.width)
                horizontalAlignment: Text.AlignHCenter; wrapMode: Text.WordWrap
                text: root.model.subtitle || ""
                color: Theme.muted; font.pixelSize: 14
            }
            Row {
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 10
                Repeater {
                    model: root.model.chips || []
                    delegate: Rectangle {
                        required property var modelData
                        height: 30; radius: 15
                        width: chipText.implicitWidth + 26
                        color: Theme.sunken
                        Accessible.role: Accessible.StaticText
                        Accessible.name: modelData.label + " " + modelData.value
                        Text {
                            id: chipText
                            anchors.centerIn: parent
                            textFormat: Text.StyledText
                            text: parent.modelData.label + "&nbsp;&nbsp;<b>" + parent.modelData.value + "</b>"
                            color: Theme.muted; font.pixelSize: 13
                        }
                    }
                }
            }

            Grid {
                width: parent.width
                columns: width > 820 ? 2 : 1
                columnSpacing: 18
                rowSpacing: 18
                topPadding: 8
                Repeater {
                    model: root.model.sections || []
                    delegate: Column {
                        id: section
                        required property var modelData
                        width: (page.width - (page.width > 820 ? 18 : 0)) / (page.width > 820 ? 2 : 1)
                        spacing: 8
                        Accessible.role: Accessible.Grouping
                        Accessible.name: modelData.title
                        Row {
                            spacing: 8
                            Icon { name: section.modelData.icon; size: 16; color: Theme.accentText }
                            Text { text: section.modelData.title; color: Theme.ink; font.pixelSize: 14; font.weight: Font.Bold
                                   Accessible.role: Accessible.Heading; Accessible.name: text }
                        }
                        Rectangle {
                            width: parent.width
                            height: inner.implicitHeight
                            radius: 14; color: Theme.card; border.color: Theme.line
                            Column {
                                id: inner
                                width: parent.width
                                // Stats: big numbers side by side.
                                Row {
                                    visible: (section.modelData.stats || []).length > 0
                                    width: parent.width
                                    Repeater {
                                        model: section.modelData.stats || []
                                        delegate: Item {
                                            required property var modelData
                                            required property int index
                                            width: inner.width / Math.max(1, (section.modelData.stats || []).length)
                                            height: 76
                                            Accessible.role: Accessible.StaticText
                                            Accessible.name: modelData.label + ": " + modelData.value
                                            Rectangle { visible: parent.index > 0; width: 1; height: parent.height; color: Theme.line }
                                            Column {
                                                x: 16; y: 14
                                                spacing: 6
                                                Text { text: parent.parent.modelData.label; color: Theme.muted; font.pixelSize: 12 }
                                                Text { text: parent.parent.modelData.value; color: Theme.ink; font.pixelSize: 20; font.weight: Font.Bold }
                                            }
                                        }
                                    }
                                }
                                Rectangle { visible: section.modelData.bar !== undefined; width: parent.width; height: 1; color: Theme.line }
                                Item {
                                    visible: section.modelData.bar !== undefined
                                    width: parent.width; height: 58
                                    Rectangle {
                                        x: 16; y: 14; width: parent.width - 32; height: 6; radius: 3; color: Theme.track
                                        Rectangle { width: parent.width * Math.min(1, section.modelData.bar || 0); height: 6; radius: 3; color: Theme.accentFill }
                                    }
                                    Text { x: 16; y: 30; text: section.modelData.caption || ""; color: Theme.muted; font.pixelSize: 12 }
                                    Text {
                                        visible: !!section.modelData.link
                                        anchors.right: parent.right; anchors.rightMargin: 16; y: 30
                                        text: section.modelData.link || ""; color: Theme.accentText; font.pixelSize: 12; font.weight: Font.DemiBold
                                        Accessible.role: Accessible.Link
                                        Accessible.name: p.t("Free Up Space")
                                        Accessible.onPressAction: p.activate("win.tool-open", "clean")
                                        TapHandler { onTapped: p.activate("win.tool-open", "clean") }
                                        HoverHandler { cursorShape: Qt.PointingHandCursor }
                                    }
                                }
                                Repeater {
                                    model: section.modelData.rows || []
                                    delegate: Item {
                                        required property var modelData
                                        width: inner.width; height: 40
                                        Accessible.role: Accessible.StaticText
                                        Accessible.name: modelData.label + ": " + modelData.value
                                        Rectangle { width: parent.width; height: 1; color: Theme.line }
                                        Text { x: 16; anchors.verticalCenter: parent.verticalCenter; text: parent.modelData.label
                                               color: Theme.ink; font.pixelSize: 13; width: parent.width * 0.45; elide: Text.ElideRight }
                                        Text { anchors.right: parent.right; anchors.rightMargin: 16; anchors.verticalCenter: parent.verticalCenter
                                               horizontalAlignment: Text.AlignRight
                                               width: parent.width * 0.5; elide: Text.ElideLeft
                                               text: parent.modelData.value; color: Theme.muted; font.pixelSize: 13 }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
