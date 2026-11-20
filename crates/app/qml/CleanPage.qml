import QtQuick
import QtQuick.Controls.Basic

// Free Up Space (P5-T02 to P5-T07; reference: smart-clean.gif): the disk
// ring, the cards in two groups, the review page with a sticky bar, the
// "Needs administrator" rows (commands to copy, never run) and the Trash.
Rectangle {
    id: root
    property var model: ({ cards: [], groups: [], disk: { looking: true }, admin: { rows: [] } })
    readonly property var review: model.review || null
    color: Theme.pane
    Accessible.role: Accessible.Pane
    Accessible.name: "Free Up Space"

    // --- overview ---------------------------------------------------------------
    Flickable {
        id: overview
        visible: !root.review
        anchors.fill: parent
        contentHeight: column.implicitHeight + 40
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

        Column {
            id: column
            x: 36; y: 26
            width: overview.width - 72
            spacing: 18

            Item {
                width: parent.width; height: heading.implicitHeight
                Column {
                    id: heading
                    width: parent.width - 160
                    spacing: 6
                    Text { text: "Free Up Space"; color: Theme.ink; font.pixelSize: 28; font.weight: Font.Bold
                           Accessible.role: Accessible.Heading; Accessible.name: text }
                    Text {
                        width: parent.width; wrapMode: Text.WordWrap; color: Theme.muted; font.pixelSize: 14
                        text: "Keep your disk roomy. Nothing goes until you have seen it; everything goes to the Trash, and Undo brings it back."
                    }
                }
                PillButton {
                    anchors.right: parent.right
                    text: "Look Again"
                    onClicked: p.activate("win.clean-look-again", "")
                }
            }

            // The disk: a ring of what fills it, the parts, and the headline.
            Rectangle {
                width: parent.width; height: 170
                radius: 18; color: Theme.card; border.color: Theme.line
                Accessible.role: Accessible.Grouping
                Accessible.name: "Disk"
                Item {
                    id: ring
                    x: 26; anchors.verticalCenter: parent.verticalCenter
                    width: 124; height: 124
                    Canvas {
                        id: ringCanvas
                        anchors.fill: parent
                        property var parts: (root.model.disk || {}).parts || []
                        onPartsChanged: requestPaint()
                        Connections { target: Theme; function onDarkChanged() { ringCanvas.requestPaint() } }
                        onPaint: {
                            const ctx = getContext("2d")
                            ctx.reset()
                            const c = width / 2, r = width / 2 - 9
                            ctx.lineWidth = 14
                            ctx.lineCap = "butt"
                            ctx.strokeStyle = Theme.track
                            ctx.beginPath(); ctx.arc(c, c, r, 0, 2 * Math.PI); ctx.stroke()
                            let start = -Math.PI / 2
                            for (const part of parts) {
                                const sweep = 2 * Math.PI * part.share
                                if (sweep <= 0.001) continue
                                ctx.strokeStyle = Theme.deep(part.color)
                                ctx.beginPath(); ctx.arc(c, c, r, start, start + sweep); ctx.stroke()
                                start += sweep
                            }
                        }
                    }
                    Column {
                        anchors.centerIn: parent
                        Text { anchors.horizontalCenter: parent.horizontalCenter
                               text: (root.model.disk || {}).looking ? "…" : root.model.disk.freeText
                               color: Theme.ink; font.pixelSize: 18; font.weight: Font.Bold }
                        Text { anchors.horizontalCenter: parent.horizontalCenter
                               text: (root.model.disk || {}).sizeText || ""
                               color: Theme.faint; font.pixelSize: 10 }
                    }
                }
                Column {
                    x: ring.x + ring.width + 30
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - x - 26
                    spacing: 12
                    Row {
                        spacing: 10
                        Text { text: (root.model.disk || {}).label || "Home disk"; color: Theme.ink
                               font.pixelSize: 17; font.weight: Font.Bold }
                        Text { text: (root.model.disk || {}).usedText || "Looking…"; color: Theme.faint; font.pixelSize: 13
                               anchors.baseline: parent.children[0].baseline }
                    }
                    Flow {
                        width: parent.width
                        spacing: 18
                        Repeater {
                            model: (root.model.disk || {}).parts || []
                            delegate: Row {
                                required property var modelData
                                spacing: 6
                                Accessible.role: Accessible.StaticText
                                Accessible.name: modelData.label + " " + modelData.text
                                Rectangle { width: 9; height: 9; radius: 4.5; color: Theme.deep(parent.modelData.color)
                                            anchors.verticalCenter: parent.verticalCenter }
                                Text { text: parent.modelData.label; color: Theme.ink; font.pixelSize: 13 }
                                Text { text: parent.modelData.text; color: Theme.faint; font.pixelSize: 13 }
                            }
                        }
                    }
                    Rectangle {
                        height: 34; radius: 10
                        width: headline.implicitWidth + 40
                        color: Theme.alpha("#2c8a57", Theme.dark ? 0.22 : 0.12)
                        border.color: Theme.alpha("#2c8a57", 0.35)
                        Accessible.role: Accessible.StaticText
                        Accessible.name: root.model.headline || ""
                        Row {
                            anchors.centerIn: parent
                            spacing: 8
                            Icon { name: "clean"; size: 15; color: "#2c8a57"; anchors.verticalCenter: parent.verticalCenter }
                            Text { id: headline; text: root.model.headline || ""; color: Theme.dark ? "#8fd6ad" : "#1f6b42"
                                   font.pixelSize: 14; font.weight: Font.DemiBold }
                        }
                    }
                }
            }

            Repeater {
                model: root.model.groups || []
                delegate: Column {
                    id: group
                    required property var modelData
                    width: column.width
                    spacing: 14
                    GroupHeading { group: group.modelData.id; label: group.modelData.label; total: group.modelData.totalText }
                    Flow {
                        width: parent.width
                        spacing: 16
                        Accessible.role: Accessible.List
                        Accessible.name: group.modelData.label
                        Repeater {
                            model: (root.model.cards || []).filter(c => c.group === group.modelData.id)
                            delegate: SuggestionCard {}
                        }
                    }
                }
            }

            // Needs administrator: sizes and commands to copy. Nothing runs.
            Column {
                width: parent.width
                spacing: 14
                visible: ((root.model.admin || {}).rows || []).length > 0
                GroupHeading { group: "admin"; label: "Needs administrator"; total: "" }
                Text {
                    width: parent.width; wrapMode: Text.WordWrap; color: Theme.muted; font.pixelSize: 13
                    text: "Cairn never runs these. Copy a command and run it in a terminal if you want to."
                }
                Rectangle {
                    width: parent.width
                    height: adminRows.implicitHeight
                    radius: 14; color: Theme.card; border.color: Theme.line
                    Accessible.role: Accessible.List
                    Accessible.name: "Needs administrator"
                    Column {
                        id: adminRows
                        width: parent.width
                        Repeater {
                            model: (root.model.admin || {}).rows || []
                            delegate: Item {
                                id: adminRow
                                required property var modelData
                                required property int index
                                width: adminRows.width; height: 74
                                Accessible.role: Accessible.ListItem
                                Accessible.name: modelData.title
                                Rectangle { visible: adminRow.index > 0; width: parent.width; height: 1; color: Theme.line }
                                Icon { x: 18; anchors.verticalCenter: parent.verticalCenter; name: "terminal"; size: 22; color: Theme.muted }
                                Column {
                                    x: 54; anchors.verticalCenter: parent.verticalCenter
                                    width: parent.width - x - 220
                                    spacing: 3
                                    Text { text: adminRow.modelData.title; color: Theme.ink; font.pixelSize: 15 }
                                    Text { text: adminRow.modelData.detail; color: Theme.faint; font.pixelSize: 12
                                           width: parent.width; elide: Text.ElideRight }
                                    Text { text: adminRow.modelData.command; color: Theme.muted; font.pixelSize: 12
                                           font.family: "monospace"; width: parent.width; elide: Text.ElideRight }
                                }
                                Text { anchors.right: copy.left; anchors.rightMargin: 16; anchors.verticalCenter: parent.verticalCenter
                                       text: adminRow.modelData.sizeText; color: Theme.ink; font.pixelSize: 15 }
                                PillButton {
                                    id: copy
                                    anchors.right: parent.right; anchors.rightMargin: 16
                                    anchors.verticalCenter: parent.verticalCenter
                                    implicitHeight: 32
                                    text: "Copy"
                                    Accessible.name: "Copy the command for " + adminRow.modelData.title
                                    onClicked: p.activate("win.clean-copy", adminRow.modelData.id)
                                }
                            }
                        }
                    }
                }
            }

            // The Trash, which is where everything here goes.
            Rectangle {
                width: parent.width; height: 64
                radius: 14; color: Theme.card; border.color: Theme.line
                Icon { x: 18; anchors.verticalCenter: parent.verticalCenter; name: "trash"; size: 22; color: Theme.muted }
                Text { x: 54; anchors.verticalCenter: parent.verticalCenter; color: Theme.ink; font.pixelSize: 15
                       text: "In the Trash: " + (root.model.trashText || "…") }
                PillButton {
                    anchors.right: parent.right; anchors.rightMargin: 16; anchors.verticalCenter: parent.verticalCenter
                    implicitHeight: 32
                    text: "Empty Trash…"
                    onClicked: p.activate("win.empty-trash", "")
                }
            }
        }
    }

    // --- review -----------------------------------------------------------------
    Item {
        id: reviewPage
        visible: !!root.review
        anchors.fill: parent
        Accessible.role: Accessible.Pane
        Accessible.name: root.review ? root.review.title : ""

        Column {
            id: reviewHead
            x: 36; y: 22
            width: parent.width - 72
            spacing: 16
            Text {
                text: "‹ All suggestions"
                color: Theme.accentText; font.pixelSize: 15
                Accessible.role: Accessible.Link
                Accessible.name: "All suggestions"
                Accessible.onPressAction: p.activate("win.clean-review", "")
                TapHandler { onTapped: p.activate("win.clean-review", "") }
                HoverHandler { cursorShape: Qt.PointingHandCursor }
            }
            Row {
                spacing: 18
                width: parent.width
                SymbolFolder { size: 68; kind: root.review ? root.review.color : "folders"
                               symbol: root.review ? root.review.symbol : "folder" }
                Column {
                    width: parent.width - 90
                    spacing: 6
                    Row {
                        spacing: 12
                        Text { text: root.review ? root.review.title : ""; color: Theme.ink; font.pixelSize: 22
                               font.weight: Font.Bold; Accessible.role: Accessible.Heading; Accessible.name: text }
                        GroupBadge { group: root.review ? root.review.group : "safe"; anchors.verticalCenter: parent.verticalCenter }
                    }
                    Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.muted; font.pixelSize: 14
                           text: root.review ? root.review.explain : "" }
                }
            }
            Row {
                spacing: 10
                TickBox {
                    checked: !!root.review && root.review.allTicked
                    label: "Tick all"
                    onToggled: p.activate("win.clean-tick", "card:" + root.review.id)
                }
                Text { text: "Tick all"; color: Theme.muted; font.pixelSize: 14; anchors.verticalCenter: parent.verticalCenter }
            }
        }

        ListView {
            id: reviewList
            anchors.top: reviewHead.bottom; anchors.topMargin: 14
            anchors.bottom: bar.top; anchors.bottomMargin: 10
            x: 36; width: parent.width - 72
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            model: root.review ? root.review.items : []
            Accessible.role: Accessible.List
            Accessible.name: "Suggestions"
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
            delegate: Rectangle {
                id: row
                required property var modelData
                required property int index
                width: reviewList.width; height: 66
                color: rowHover.hovered ? Theme.hover : Theme.card
                border.color: Theme.line
                radius: 0
                Accessible.role: Accessible.ListItem
                Accessible.name: modelData.title
                HoverHandler { id: rowHover }
                TickBox {
                    x: 18; anchors.verticalCenter: parent.verticalCenter
                    visible: !row.modelData.keep
                    checked: row.modelData.ticked
                    label: "Tick " + row.modelData.title
                    onToggled: p.activate("win.clean-tick", row.modelData.path)
                }
                Rectangle {
                    visible: row.modelData.keep
                    x: 12; anchors.verticalCenter: parent.verticalCenter
                    width: 40; height: 22; radius: 11
                    color: Theme.alpha(Theme.accent, 0.14)
                    Accessible.role: Accessible.StaticText
                    Accessible.name: "Keep " + row.modelData.title
                    Text { anchors.centerIn: parent; text: "Keep"; color: Theme.accentText; font.pixelSize: 11; font.weight: Font.DemiBold }
                }
                Column {
                    x: 60; anchors.verticalCenter: parent.verticalCenter
                    width: parent.width * 0.42
                    spacing: 2
                    Text { width: parent.width; text: row.modelData.title; color: Theme.ink; font.pixelSize: 15; elide: Text.ElideMiddle }
                    Text { width: parent.width; text: row.modelData.where; color: Theme.faint; font.pixelSize: 12; elide: Text.ElideMiddle }
                }
                Row {
                    anchors.right: size.left; anchors.rightMargin: 18
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 6
                    IconButton { width: 28; height: 28; iconSize: 15; flat: true; icon: "preview"
                                 visible: rowHover.hovered; label: "Peek at " + row.modelData.title
                                 onClicked: p.activate("win.clean-peek", row.modelData.path) }
                    IconButton { width: 28; height: 28; iconSize: 15; flat: true; icon: "folder"
                                 visible: rowHover.hovered; label: "Show " + row.modelData.title + " in its folder"
                                 onClicked: p.activate("win.clean-show", row.modelData.path) }
                    Text { text: row.modelData.why; color: Theme.muted; font.pixelSize: 13
                           anchors.verticalCenter: parent.verticalCenter
                           width: Math.min(implicitWidth, reviewList.width * 0.3); elide: Text.ElideRight }
                }
                Text { id: size; anchors.right: parent.right; anchors.rightMargin: 20; anchors.verticalCenter: parent.verticalCenter
                       text: row.modelData.sizeText; color: Theme.ink; font.pixelSize: 15 }
            }
        }

        // The sticky bar: what is ticked, and Move to Trash.
        Rectangle {
            id: bar
            anchors.bottom: parent.bottom
            width: parent.width; height: 70
            color: Theme.pane
            Rectangle { width: parent.width; height: 1; color: Theme.line }
            Text { x: 36; anchors.verticalCenter: parent.verticalCenter; color: Theme.muted; font.pixelSize: 15
                   text: root.review ? root.review.tickedText : "" }
            PillButton {
                anchors.right: parent.right; anchors.rightMargin: 36; anchors.verticalCenter: parent.verticalCenter
                primary: true
                text: "Move to Trash"
                enabled: !!root.review && root.review.tickedCount > 0
                Accessible.name: "Move ticked items to the Trash"
                onClicked: p.activate("win.clean-trash", root.review.id)
            }
        }
    }

    // --- parts ------------------------------------------------------------------
    component GroupBadge: Rectangle {
        property string group: "safe"
        readonly property color tone: group === "safe" ? "#2c8a57" : (group === "admin" ? "#4c5361" : "#b8740a")
        width: badgeRow.implicitWidth + 16; height: 22; radius: 11
        color: Theme.alpha(tone, Theme.dark ? 0.25 : 0.12)
        border.color: Theme.alpha(tone, 0.35)
        Row {
            id: badgeRow
            anchors.centerIn: parent
            spacing: 4
            Icon { name: "shield"; size: 11; color: parent.parent.tone; visible: parent.parent.group === "safe"
                   anchors.verticalCenter: parent.verticalCenter }
            Text { text: parent.parent.group === "safe" ? "Safe to clear" : (parent.parent.group === "admin" ? "Needs administrator" : "Your call")
                   color: parent.parent.tone; font.pixelSize: 11; font.weight: Font.DemiBold }
        }
    }

    component GroupHeading: Item {
        property string group: "safe"
        property string label: ""
        property string total: ""
        width: column.width; height: 24
        Accessible.role: Accessible.Heading
        Accessible.name: label
        Row {
            id: headingRow
            spacing: 10
            anchors.verticalCenter: parent.verticalCenter
            GroupBadge { group: parent.parent.group }
            Text { text: parent.parent.total; color: Theme.ink; font.pixelSize: 14; font.weight: Font.Bold
                   anchors.verticalCenter: parent.verticalCenter }
        }
        Rectangle { anchors.left: headingRow.right; anchors.leftMargin: 14; anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter; height: 1; color: Theme.line }
    }

    component SuggestionCard: Rectangle {
        id: card
        required property var modelData
        readonly property bool looking: modelData.state === "looking"
        width: Math.max(220, (column.width - 32) / 3)
        height: 196
        radius: 18
        color: Theme.card
        border.color: cardHover.hovered ? Theme.alpha(Theme.accent, 0.4) : Theme.line
        Accessible.role: Accessible.Button
        Accessible.name: modelData.title
        Accessible.description: modelData.subtitle + ", " + (looking ? "looking" : modelData.totalText)
        Accessible.onPressAction: p.activate("win.clean-review", modelData.id)
        HoverHandler { id: cardHover; cursorShape: Qt.PointingHandCursor }
        TapHandler { onTapped: p.activate("win.clean-review", card.modelData.id) }
        SymbolFolder { anchors.horizontalCenter: parent.horizontalCenter; y: 20; size: 52
                       kind: card.modelData.color; symbol: card.modelData.symbol }
        Column {
            y: 76; width: parent.width
            spacing: 4
            Text { anchors.horizontalCenter: parent.horizontalCenter; text: card.modelData.title
                   color: Theme.ink; font.pixelSize: 15; font.weight: Font.Bold }
            Text { anchors.horizontalCenter: parent.horizontalCenter; text: card.modelData.subtitle
                   color: Theme.muted; font.pixelSize: 12
                   SequentialAnimation on opacity {
                       running: card.looking && Theme.motion; loops: Animation.Infinite
                       NumberAnimation { to: 0.35; duration: 600 } NumberAnimation { to: 1; duration: 600 }
                   }
            }
        }
        Rectangle { x: 16; y: 138; width: parent.width - 32; height: 1; color: Theme.line }
        Text { x: 18; y: 156; text: card.looking ? "—" : card.modelData.totalText; color: Theme.ink
               font.pixelSize: 16; font.weight: Font.Bold }
        GroupBadge { anchors.right: parent.right; anchors.rightMargin: 16; y: 154; group: card.modelData.group }
    }
}
