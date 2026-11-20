import QtQuick

// A file card (DESIGN.md §3.2): preview area (thumbnail or tinted art),
// coloured type badge, name, kind and size.
Item {
    id: root

    property string name: ""
    property string meta: ""
    property string badge: ""
    property string kind: "other"
    property string symbol: "document"
    property string thumb: ""
    property bool selected: false
    readonly property bool lifted: hover.hovered
    readonly property color tint: Theme.deep(kind)
    property string uri: ""
    property bool renaming: false
    signal tapped(int modifiers)
    signal doubleTapped()
    signal menuRequested()

    width: 196
    height: Math.round(width * 0.8)

    Accessible.role: Accessible.Button
    Accessible.name: name
    Accessible.selected: selected
    Accessible.onPressAction: root.doubleTapped()

    HoverHandler { id: hover; cursorShape: Qt.PointingHandCursor }
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

    Rectangle {
        id: card
        width: parent.width
        height: parent.height
        y: root.lifted ? -4 : 0
        Behavior on y { enabled: Theme.motion; SpringAnimation { spring: 4; damping: 0.3 } }
        radius: 14
        color: Theme.card
        border.color: root.selected ? Theme.accent
                                    : (root.lifted ? Theme.alpha(Theme.accent, 0.55) : Theme.line)
        border.width: root.selected ? 2 : 1
        Behavior on border.color { ColorAnimation { duration: 160 } }

        layer.enabled: Theme.effects
        layer.samplerName: "tex"
        layer.effect: FxShadow {
            shadowColor: "#1a1240"
            shadowOpacity: root.lifted ? 0.22 : 0.06
            shadowBlur: root.lifted ? 0.8 : 0.3
            shadowVerticalOffset: root.lifted ? 8 : 1
            Behavior on shadowOpacity { NumberAnimation { duration: 180 } }
        }

        Rectangle {
            id: preview
            x: 1; y: 1
            width: parent.width - 2
            height: parent.height - 58
            radius: 13
            color: Theme.sunken
            clip: true
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 14; color: parent.color }

            Rectangle {
                visible: image.status !== Image.Ready
                anchors.centerIn: parent
                width: parent.width * 0.6; height: parent.height * 0.62; radius: 8
                gradient: Gradient {
                    orientation: Gradient.Horizontal
                    GradientStop { position: 0.0; color: Theme.light(root.kind) }
                    GradientStop { position: 1.0; color: root.tint }
                }
                opacity: 0.85
                Icon { anchors.centerIn: parent; name: root.symbol; size: Math.round(parent.height * 0.42); color: "white" }
            }
            Image {
                id: image
                anchors.fill: parent
                source: root.thumb ? "file://" + root.thumb : ""
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
                sourceSize.width: width * 2
                opacity: status === Image.Ready ? 1 : 0
                Behavior on opacity { NumberAnimation { duration: 180 } }
            }
        }
        // A small page with the type on a coloured label (reference: design.png).
        Item {
            id: badgeBox
            x: 12; anchors.verticalCenter: info.verticalCenter
            width: 26; height: 30
            Rectangle {
                x: 3; width: 20; height: 26; radius: 3
                color: Theme.card; border.color: Theme.line; border.width: 1
                Rectangle { x: 13; width: 7; height: 7; color: Theme.sunken; border.color: Theme.line; border.width: 1 }
            }
            Rectangle {
                y: 15; width: parent.width; height: 11; radius: 3
                color: root.tint
                Text { anchors.centerIn: parent; text: root.badge.slice(0, 4); color: "white"; font.pixelSize: 7; font.weight: Font.Black }
            }
        }
        IconButton {
            id: more
            anchors.right: parent.right; anchors.rightMargin: 6
            anchors.verticalCenter: info.verticalCenter
            width: 26; height: 26; iconSize: 15
            flat: true
            icon: "more"
            rotation: 90
            label: p.t("More for ") + root.name
            opacity: root.lifted || root.selected ? 1 : 0.55
            onClicked: { root.tapped(0); root.menuRequested() }
        }
        Column {
            id: info
            anchors.left: badgeBox.right; anchors.leftMargin: 9
            anchors.right: more.left; anchors.rightMargin: 4
            y: preview.height + 12
            Text { width: parent.width; text: root.name; elide: Text.ElideMiddle; color: Theme.ink
                   font.pixelSize: 13; font.weight: Font.Medium; visible: !root.renaming }
            Loader {
                active: root.renaming
                visible: active
                width: parent.width
                sourceComponent: RenameField { uri: root.uri; original: root.name; horizontalAlignment: TextInput.AlignLeft }
            }
            Text { width: parent.width; text: root.meta; elide: Text.ElideRight; color: Theme.muted; font.pixelSize: 11 }
        }
    }
}
