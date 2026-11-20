import QtQuick
import QtQuick.Controls.Basic

// The list view (DESIGN.md §3.3): Name, Kind, Size, Modified columns with a
// section label when grouping.
Item {
    id: root
    property var items: []
    property var selection: ({})
    property string renaming: ""
    property int anchor: -1
    signal itemMenu(int pos)
    onAnchorChanged: if (anchor >= 0 && anchor < items.length) list.positionViewAtIndex(anchor, ListView.Contain)

    Accessible.role: Accessible.List
    Accessible.name: p.t("Folder contents")

    function modes(modifiers) {
        if (modifiers & Qt.ControlModifier) return 1
        if (modifiers & Qt.ShiftModifier) return 2
        return 0
    }

    readonly property real kindX: width * 0.50
    readonly property real sizeX: width * 0.68
    readonly property real dateX: width * 0.80

    Item {
        id: heading
        width: parent.width
        height: 34
        Repeater {
            model: [[p.t("Name"), 54], [p.t("Kind"), root.kindX], [p.t("Size"), root.sizeX], [p.t("Modified"), root.dateX]]
            delegate: Text {
                required property var modelData
                x: modelData[1]
                anchors.verticalCenter: parent.verticalCenter
                text: modelData[0]
                color: Theme.faint
                font.pixelSize: 12
                font.weight: Font.DemiBold
            }
        }
        Rectangle { anchors.bottom: parent.bottom; x: 22; width: parent.width - 44; height: 1; color: Theme.line }
    }

    ListView {
        id: list
        anchors.top: heading.bottom
        anchors.left: parent.left; anchors.right: parent.right; anchors.bottom: parent.bottom
        clip: true
        model: root.items
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
        delegate: Item {
            id: slot
            required property var modelData
            required property int index
            readonly property bool opensSection: !!modelData.section
                && (index === 0 || root.items[index - 1].section !== modelData.section)
            width: list.width
            height: line.height + (opensSection ? 36 : 0)
            Text {
                visible: slot.opensSection
                x: 24; y: 12
                text: slot.modelData.section
                color: Theme.ink
                font.pixelSize: 13; font.weight: Font.Bold
                Accessible.role: Accessible.Heading
                Accessible.name: text
            }
        Rectangle {
            id: line
            readonly property var modelData: slot.modelData
            readonly property int index: slot.index
            readonly property bool selected: !!root.selection[modelData.pos]
            x: 12
            y: slot.opensSection ? 36 : 0
            width: list.width - 24
            height: 36
            radius: 9
            color: selected ? Theme.alpha(Theme.accent, 0.22) : (hover.hovered ? Theme.hover : (index % 2 ? Theme.alpha(Theme.ink, 0.025) : "transparent"))
            Accessible.role: Accessible.ListItem
            Accessible.name: modelData.name
            Accessible.selected: selected
            Accessible.onPressAction: p.open(modelData.pos)
            HoverHandler { id: hover }
            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.LeftButton | Qt.RightButton
                onClicked: mouse => {
                    if (mouse.button === Qt.RightButton) root.itemMenu(line.modelData.pos)
                    else p.pick(line.modelData.pos, root.modes(mouse.modifiers))
                }
                onDoubleClicked: mouse => { if (mouse.button === Qt.LeftButton) p.open(line.modelData.pos) }
            }
            Rectangle {
                x: 12; anchors.verticalCenter: parent.verticalCenter
                width: 22; height: 22; radius: 6
                color: Theme.alpha(Theme.deep(line.modelData.kind), 0.16)
                Icon {
                    anchors.centerIn: parent; size: 14
                    name: line.modelData.dir ? "folder" : ({ images: "pictures", videos: "videos", audio: "music", pdfs: "pdf",
                          archives: "box", installers: "package", code: "code" })[line.modelData.kind] || "document"
                    color: Theme.deep(line.modelData.kind)
                }
            }
            Text { x: 42; width: root.kindX - 54; anchors.verticalCenter: parent.verticalCenter; text: line.modelData.name
                   elide: Text.ElideMiddle; font.pixelSize: 13; color: Theme.ink
                   visible: root.renaming !== line.modelData.uri }
            Loader {
                active: root.renaming === line.modelData.uri
                x: 38; width: root.kindX - 46; height: 28
                anchors.verticalCenter: parent.verticalCenter
                sourceComponent: RenameField {
                    uri: line.modelData.uri; original: line.modelData.name; isFolder: line.modelData.dir
                    horizontalAlignment: TextInput.AlignLeft
                }
            }
            Text { x: root.kindX - 12; width: root.sizeX - root.kindX - 8; anchors.verticalCenter: parent.verticalCenter
                   text: line.modelData.kindText; elide: Text.ElideRight; font.pixelSize: 13
                   color: Theme.muted }
            Text { x: root.sizeX - 12; width: root.dateX - root.sizeX - 8; anchors.verticalCenter: parent.verticalCenter
                   text: line.modelData.sizeText; font.pixelSize: 13; color: Theme.muted }
            Text { x: root.dateX - 12; width: parent.width - x - 12; anchors.verticalCenter: parent.verticalCenter
                   text: line.modelData.modified; elide: Text.ElideRight; font.pixelSize: 13
                   color: Theme.muted }
        }
        }
    }
}
