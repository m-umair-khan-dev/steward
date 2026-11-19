import QtQuick
import QtQuick.Controls.Basic

// The grid (DESIGN.md §3.3, §3.5): section headers, rows of glass folders and
// file cards, and kind folders when grouping by kind. Rows are built only near
// the screen. Switching to "Kind" flies the visible file cards into their
// kind folders, which spring in.
Item {
    id: root
    property var rows: []
    property var selection: ({})
    property var thumbs: ({})
    property var counts: ({})
    property var metrics: ({ folder: 168, file: 240, gap: 18 })
    property string groupBy: "none"
    property string renaming: ""
    property int anchor: -1
    property bool previewShown: false
    signal itemMenu(int pos)

    // Keep the item the keyboard is on (and one being renamed) on screen.
    function reveal(pos) {
        if (pos < 0) return
        const index = shown.findIndex(r => r.t === "c" && r.items.some(i => i.pos === pos))
        if (index >= 0) list.positionViewAtIndex(index, ListView.Contain)
    }
    onAnchorChanged: reveal(anchor)
    onRenamingChanged: reveal(anchor)

    readonly property int margin: 22
    readonly property int gap: metrics.gap || 18
    property var shown: []
    property string shownGroup: "none"
    property bool springKinds: false
    property var liveCards: ({})
    property var liveKinds: ({})
    property int inFlight: 0
    onInFlightChanged: p.flying(inFlight)

    Accessible.role: Accessible.List
    Accessible.name: "Folder contents"

    function modes(modifiers) {
        if (modifiers & Qt.ControlModifier) return 1
        if (modifiers & Qt.ShiftModifier) return 2
        return 0
    }

    onRowsChanged: {
        let ghosts = []
        if (Theme.motion && groupBy === "kind" && shownGroup !== "kind")
            ghosts = captureGhosts()
        springKinds = ghosts.length > 0 || (Theme.motion && groupBy === "kind" && shownGroup !== "kind")
        shownGroup = groupBy
        shown = rows
        Qt.callLater(reveal, anchor)
        if (ghosts.length)
            Qt.callLater(launchGhosts, ghosts)
        if (springKinds)
            springOff.restart()
    }
    onWidthChanged: p.resize(width - 2 * margin)
    Timer { id: springOff; interval: 1400; onTriggered: root.springKinds = false }

    function captureGhosts() {
        const out = []
        for (const key in liveCards) {
            const card = liveCards[key]
            if (!card || card.isFolder) continue
            const at = card.mapToItem(root, 0, 0)
            if (at.y + card.height < 0 || at.y > root.height) continue
            out.push({ name: card.name, meta: card.meta, badge: card.badge, kind: card.kind, symbol: card.symbol,
                       thumb: card.thumb, x: at.x, y: at.y, w: card.width })
            if (out.length >= 60) break
        }
        out.sort((a, b) => a.y - b.y || a.x - b.x)
        return out
    }

    function launchGhosts(ghosts) {
        list.forceLayout()
        ghosts.forEach((g, i) => {
            const target = liveKinds[g.kind]
            let tx = g.x, ty = root.height
            if (target) {
                const at = target.mapToItem(root, target.width / 2, target.width * 0.4)
                tx = at.x - g.w / 2
                ty = at.y - g.w * 0.4
            }
            ghost.createObject(root, { ghost: g, x: g.x, y: g.y, width: g.w, tx: tx, ty: ty, delay: Math.min(i * 22, 420) })
        })
    }

    Component {
        id: ghost
        FileCard {
            id: flyer
            property var ghost
            property real tx
            property real ty
            property int delay
            z: 10
            name: ghost.name; meta: ghost.meta; badge: ghost.badge; kind: ghost.kind
            symbol: ghost.symbol; thumb: ghost.thumb
            Accessible.ignored: true
            enabled: false
            Component.onCompleted: root.inFlight += 1
            Component.onDestruction: root.inFlight -= 1
            SequentialAnimation {
                running: true
                PauseAnimation { duration: flyer.delay }
                ParallelAnimation {
                    NumberAnimation { target: flyer; property: "x"; to: flyer.tx; duration: 560; easing.type: Easing.InOutCubic }
                    NumberAnimation { target: flyer; property: "y"; to: flyer.ty; duration: 560; easing.type: Easing.InOutCubic }
                    NumberAnimation { target: flyer; property: "scale"; to: 0.18; duration: 560; easing.type: Easing.InOutCubic }
                    SequentialAnimation {
                        PauseAnimation { duration: 380 }
                        NumberAnimation { target: flyer; property: "opacity"; to: 0; duration: 180 }
                    }
                }
                ScriptAction { script: flyer.destroy() }
            }
        }
    }

    ListView {
        id: list
        anchors.fill: parent
        clip: true
        model: root.shown
        cacheBuffer: 400
        boundsBehavior: Flickable.StopAtBounds
        topMargin: 4
        bottomMargin: 24
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

        delegate: Item {
            id: row
            required property var modelData
            required property int index
            width: list.width
            height: modelData.t === "h" ? 46 : (loader.item ? loader.item.height + root.gap : 0)
            Component.onCompleted: p.rowbound(1)
            Component.onDestruction: p.rowbound(-1)

            Loader {
                id: loader
                x: root.margin
                width: parent.width - 2 * root.margin
                sourceComponent: row.modelData.t === "h" ? header : (row.modelData.t === "k" ? kinds : cards)
                property var rowData: row.modelData
            }
        }
    }

    Component {
        id: header
        Item {
            height: 46
            Text {
                anchors.bottom: parent.bottom
                anchors.bottomMargin: 12
                textFormat: Text.StyledText
                text: "<b>" + parent.parent.rowData.title + "</b>&nbsp;&nbsp;<font color='" + Theme.faint + "'>"
                      + parent.parent.rowData.count + "</font>"
                color: Theme.ink
                font.pixelSize: 15
                Accessible.role: Accessible.Heading
                Accessible.name: parent.parent.rowData.title
            }
        }
    }

    Component {
        id: cards
        Row {
            id: cardRow
            readonly property var rowData: parent.rowData
            spacing: root.gap
            Repeater {
                model: cardRow.rowData.items
                delegate: Loader {
                    id: slot
                    required property var modelData
                    sourceComponent: modelData.dir ? folderCard : fileCard
                    readonly property bool isFolder: modelData.dir
                    readonly property string name: modelData.name
                    readonly property string meta: modelData.meta
                    readonly property string badge: modelData.badge
                    readonly property string kind: modelData.kind
                    readonly property string symbol: modelData.symbol
                    readonly property string thumb: root.thumbs[modelData.uri] || ""
                    // Folders: "N items" and the size from the index ("—" while unknown).
                    readonly property string count: (root.counts[modelData.uri] || {}).count || ""
                    readonly property string size: (root.counts[modelData.uri] || {}).size || ""
                    readonly property bool selected: !!root.selection[modelData.pos]
                    onThumbChanged: p.card(name, thumb !== "", count, 0)
                    onCountChanged: p.card(name, thumb !== "", count, 0)
                    Component.onCompleted: {
                        root.liveCards[modelData.pos] = slot
                        p.card(name, thumb !== "", count, 1)
                        p.seen(modelData.uri, true)
                    }
                    Component.onDestruction: {
                        if (root.liveCards[modelData.pos] === slot) delete root.liveCards[modelData.pos]
                        p.card(name, false, "", -1)
                        p.seen(modelData.uri, false)
                    }
                }
            }
        }
    }

    Component {
        id: folderCard
        GlassFolder {
            width: root.metrics.folder || 168
            kind: "folders"
            label: parent.name
            count: parent.count
            size: parent.size
            symbol: parent.symbol
            selected: parent.selected
            uri: parent.modelData.uri
            renaming: root.renaming === parent.modelData.uri
            onTapped: modifiers => p.pick(parent.modelData.pos, root.modes(modifiers))
            onDoubleTapped: p.open(parent.modelData.pos)
            onMenuRequested: root.itemMenu(parent.modelData.pos)
            onInfoRequested: {
                p.pick(parent.modelData.pos, 0)
                if (!root.previewShown) p.activate("win.toggle-preview", "")
            }
        }
    }

    Component {
        id: fileCard
        FileCard {
            width: root.metrics.file || 240
            name: parent.name
            meta: parent.meta
            badge: parent.badge
            kind: parent.kind
            symbol: parent.symbol
            thumb: parent.thumb
            selected: parent.selected
            uri: parent.modelData.uri
            renaming: root.renaming === parent.modelData.uri
            onTapped: modifiers => p.pick(parent.modelData.pos, root.modes(modifiers))
            onDoubleTapped: p.open(parent.modelData.pos)
            onMenuRequested: root.itemMenu(parent.modelData.pos)
        }
    }

    Component {
        id: kinds
        Row {
            id: kindRow
            readonly property var rowData: parent.rowData
            spacing: root.gap
            Repeater {
                model: kindRow.rowData.kinds
                delegate: GlassFolder {
                    id: kindFolder
                    required property var modelData
                    required property int index
                    width: root.metrics.folder || 168
                    kind: modelData.kind
                    label: modelData.label
                    count: modelData.countText
                    size: modelData.sizeText
                    symbol: ({ images: "pictures", videos: "videos", audio: "music", pdfs: "pdf", documents: "document",
                               archives: "box", installers: "package", code: "code" })[modelData.kind] || "more"
                    onTapped: p.activate("win.filter-kind", modelData.kind)
                    onDoubleTapped: p.activate("win.filter-kind", modelData.kind)
                    transformOrigin: Item.Center
                    Component.onCompleted: {
                        root.liveKinds[modelData.kind] = kindFolder
                        if (root.springKinds) { scale = 0.55; opacity = 0; spring.start() }
                    }
                    Component.onDestruction: if (root.liveKinds[modelData.kind] === kindFolder) delete root.liveKinds[modelData.kind]
                    SequentialAnimation {
                        id: spring
                        PauseAnimation { duration: 260 + kindFolder.index * 60 }
                        ParallelAnimation {
                            NumberAnimation { target: kindFolder; property: "opacity"; to: 1; duration: 220 }
                            SpringAnimation { target: kindFolder; property: "scale"; to: 1; spring: 3.2; damping: 0.22; epsilon: 0.005 }
                        }
                    }
                }
            }
        }
    }
}
