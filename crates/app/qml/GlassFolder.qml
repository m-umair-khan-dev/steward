import QtQuick

// A tinted glass folder (ADR 0008, DESIGN.md §3.2): coloured back with a tab,
// two sheets of paper, and a frosted front that really blurs what is behind.
Item {
    id: root

    property string kind: "folders"
    property string label: ""
    property string count: ""
    property string size: ""
    property string symbol: "document"
    property bool selected: false
    readonly property bool lifted: hover.hovered || selected || drop.containsDrag
    property bool acceptsDrops: true
    property string uri: ""
    property bool renaming: false
    signal tapped(int modifiers)
    signal doubleTapped()
    signal menuRequested()
    signal infoRequested()

    readonly property color deep: Theme.deep(kind)
    readonly property color light: Theme.light(kind)

    width: 148
    height: shape.height + 44

    Accessible.role: Accessible.Button
    Accessible.name: label
    Accessible.selected: selected
    Accessible.onPressAction: root.doubleTapped()

    HoverHandler { id: hover; cursorShape: Qt.PointingHandCursor }
    FolderDrop { id: drop; anchors.fill: parent; target: root.uri; enabled: root.acceptsDrops && root.uri !== "" }
    // A MouseArea (not a TapHandler) so the click stops here and never also
    // reaches the background behind the card.
    // Dragging the card carries it (and the rest of the selection).
    DragSource {
        uri: root.uri
        onClicked: mouse => {
            if (mouse.button === Qt.RightButton) root.menuRequested()
            else root.tapped(mouse.modifiers)
        }
        onDoubleClicked: mouse => { if (mouse.button === Qt.LeftButton) root.doubleTapped() }
    }

    Item {
        id: shape
        width: root.width - 16
        height: Math.round(width * 0.8)
        anchors.horizontalCenter: parent.horizontalCenter
        y: root.lifted ? 0 : 4
        scale: hover.hovered ? 1.05 : 1.0
        Behavior on y { enabled: Theme.motion; SpringAnimation { spring: 4; damping: 0.32 } }
        Behavior on scale { enabled: Theme.motion; SpringAnimation { spring: 4; damping: 0.32 } }

        layer.enabled: Theme.effects
        layer.samplerName: "tex"
        layer.effect: FxShadow {
            shadowColor: Theme.alpha(root.deep, 0.55)
            shadowBlur: root.lifted ? 0.9 : 0.5
            shadowVerticalOffset: root.lifted ? 8 : 3
            shadowOpacity: root.lifted ? 0.55 : 0.3
            Behavior on shadowBlur { NumberAnimation { duration: 180 } }
            Behavior on shadowVerticalOffset { NumberAnimation { duration: 180 } }
        }

        Item {
            id: behind
            anchors.fill: parent
            // The tab, with a slanted right edge (reference).
            Rectangle { width: parent.width * 0.34; height: parent.height * 0.25; radius: 10; color: root.deep }
            Rectangle {
                x: parent.width * 0.25; y: parent.height * 0.035
                width: parent.width * 0.18; height: parent.height * 0.2; radius: 6
                rotation: 38; transformOrigin: Item.BottomLeft
                color: root.deep; antialiasing: true
            }
            Rectangle { y: parent.height * 0.11; width: parent.width; height: parent.height * 0.89; radius: 14; color: root.deep }
            Rectangle { x: parent.width * 0.14; y: parent.height * 0.17; width: parent.width * 0.72; height: parent.height * 0.5
                        radius: 6; color: Theme.fillInk(1); rotation: -4; antialiasing: true }
            Rectangle { x: parent.width * 0.15; y: parent.height * 0.21; width: parent.width * 0.72; height: parent.height * 0.5
                        radius: 6; color: Theme.paper; rotation: 3; antialiasing: true }
        }

        ShaderEffectSource {
            id: behindTexture
            live: Theme.effects
            sourceItem: behind
            sourceRect: Qt.rect(front.x, front.y, front.width, front.height)
            visible: false
        }
        Rectangle {
            id: frontMask
            width: front.width; height: front.height; radius: front.radius
            visible: false
            layer.enabled: true
        }
        // Frosted glass on Qt 6.5+; on Qt 6.4 the front stays translucent.
        FxBlur {
            x: front.x; y: front.y; width: front.width; height: front.height
            source: behindTexture
            mask: frontMask
        }
        Rectangle {
            id: front
            y: parent.height * 0.36
            width: parent.width
            height: parent.height - y
            radius: 12
            gradient: Gradient {
                GradientStop { position: 0.0; color: Theme.alpha(root.light, 0.70) }
                GradientStop { position: 1.0; color: Theme.alpha(root.deep, 0.80) }
            }
            border.color: Theme.fillInk(0.45)
            border.width: 1

            Text {
                x: 10; y: 6
                text: root.count
                color: Theme.accentInk; font.pixelSize: 11; font.weight: Font.Bold
                width: parent.width - 20; elide: Text.ElideRight
            }
            // "i": select the folder and show it in the preview pane.
            Item {
                visible: root.uri !== ""
                anchors.right: parent.right; anchors.rightMargin: 7
                y: 5
                width: 16; height: 16
                Accessible.role: Accessible.Button
                Accessible.name: p.t("About ") + root.label
                Accessible.onPressAction: root.infoRequested()
                Icon { anchors.fill: parent; name: "info"; size: 16; color: Theme.fillInk(0.92) }
                MouseArea { anchors.fill: parent; anchors.margins: -4; onClicked: root.infoRequested() }
            }
            Icon {
                anchors.centerIn: parent
                anchors.verticalCenterOffset: 2
                name: root.symbol
                size: Math.round(front.height * 0.34)
                color: Theme.fillInk(0.92)
            }
            Text {
                x: 10; anchors.bottom: parent.bottom; anchors.bottomMargin: 5
                text: root.size
                color: Theme.fillInk(0.92); font.pixelSize: 10
            }
        }
    }

    Loader {
        active: root.renaming
        z: 2
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: shape.bottom
        anchors.topMargin: 6
        width: root.width + 12
        sourceComponent: RenameField { uri: root.uri; original: root.label; isFolder: true }
    }
    Rectangle {
        visible: !root.renaming
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: shape.bottom
        anchors.topMargin: 8
        width: Math.min(name.implicitWidth + 14, root.width)
        height: 24
        radius: 8
        color: root.selected ? Theme.accentFill : "transparent"
        Text {
            id: name
            anchors.centerIn: parent
            width: Math.min(implicitWidth, root.width - 14)
            text: root.label
            color: root.selected ? Theme.accentInk : Theme.ink
            font.pixelSize: 13
            font.weight: Font.Medium
            elide: Text.ElideMiddle
            horizontalAlignment: Text.AlignHCenter
        }
    }
}
