import QtQuick
import QtQuick.Controls.Basic

// Bookmarks (P7-T08; reference: bookmarks.gif): links with their pictures,
// four views (Grid, List, Headlines, Masonry), tags to filter by. Paste a
// link (Ctrl+V), drop one onto the page, or Add Link.
Rectangle {
    id: root
    property var model: ({ cards: [], tags: [] })
    readonly property var cards: model.cards || []
    readonly property string view: model.view || "grid"
    color: Theme.pane
    Accessible.role: Accessible.Pane
    Accessible.name: p.t("Bookmarks")

    // A link dropped from a browser or another app.
    DropArea {
        anchors.fill: parent
        keys: ["text/uri-list", "text/plain", "text/x-moz-url"]
        onEntered: drag => { drag.accept(Qt.CopyAction) }
        onDropped: drop => {
            const link = drop.hasUrls ? String(drop.urls[0]) : (drop.hasText ? drop.text.split("\n")[0] : "")
            if (link) p.activate("win.bookmark-add", link)
            drop.accept(Qt.CopyAction)
        }
        Rectangle {
            anchors.fill: parent; anchors.margins: 10
            visible: parent.containsDrag
            radius: 18; color: Theme.alpha(Theme.accent, 0.08)
            border.color: Theme.accent; border.width: 2
            z: 50
        }
    }

    // --- top bar ---------------------------------------------------------------
    Item {
        id: bar
        width: parent.width; height: 74
        Row {
            x: 28; anchors.verticalCenter: parent.verticalCenter
            spacing: 12
            Text { id: heading; text: root.model.title || ""; color: Theme.ink; font.pixelSize: 24; font.weight: Font.Bold
                   Accessible.role: Accessible.Heading; Accessible.name: text }
            Text { text: (root.model.summary || "") + " · " + ({ grid: p.t("Grid"), list: p.t("List"),
                         headlines: p.t("Headlines"), masonry: p.t("Masonry") })[root.view]
                   color: Theme.faint; font.pixelSize: 14; anchors.baseline: heading.baseline }
        }
        Row {
            anchors.right: parent.right; anchors.rightMargin: 24; anchors.verticalCenter: parent.verticalCenter
            spacing: 10
            PillButton { visible: root.model.collection !== null && root.model.collection !== undefined
                         implicitHeight: 34; text: p.t("Delete Collection")
                         onClicked: p.activate("win.collection-delete", String(root.model.collection)) }
            Segmented {
                name: p.t("View")
                options: [{ id: "grid", icon: "grid", a11y: p.t("Grid") }, { id: "list", icon: "list", a11y: p.t("List") },
                          { id: "headlines", icon: "headlines", a11y: p.t("Headlines") },
                          { id: "masonry", icon: "masonry", a11y: p.t("Masonry") }]
                current: root.view
                onPicked: id => p.activate("win.bookmarks-view", id)
            }
            PillButton { implicitHeight: 34; primary: true; text: p.t("Add Link")
                         onClicked: p.activate("win.bookmark-add-ask", "") }
        }
    }

    // Tags to filter by.
    Flow {
        id: tagRow
        x: 28; anchors.top: bar.bottom
        width: parent.width - 56
        spacing: 8
        visible: (root.model.tags || []).length > 0
        height: visible ? implicitHeight + 12 : 0
        Accessible.role: Accessible.List
        Accessible.name: p.t("Tags")
        Repeater {
            model: root.model.tags || []
            delegate: Rectangle {
                required property string modelData
                readonly property bool on: root.model.tag === modelData
                height: 28; radius: 14; width: tagText.implicitWidth + 22
                color: on ? Theme.accentFill : Theme.sunken
                Accessible.role: Accessible.Button
                Accessible.name: "#" + modelData
                Accessible.checked: on
                Accessible.onPressAction: p.activate("win.bookmarks-tag", modelData)
                Text { id: tagText; anchors.centerIn: parent; text: "#" + parent.modelData
                       color: parent.on ? Theme.accentInk : Theme.ink; font.pixelSize: 13 }
                TapHandler { onTapped: p.activate("win.bookmarks-tag", parent.modelData) }
            }
        }
    }

    Text {
        anchors.centerIn: parent
        width: Math.min(parent.width - 80, 460)
        visible: root.cards.length === 0
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.WordWrap
        text: p.t("No links here yet. Paste a link with Ctrl+V, drop one onto this page, or choose Add Link.")
        color: Theme.faint; font.pixelSize: 15
    }

    // --- the links ----------------------------------------------------------------
    Flickable {
        id: flick
        anchors.top: tagRow.bottom; anchors.bottom: parent.bottom
        width: parent.width
        contentHeight: (root.view === "masonry" ? masonry.height : list.height) + 30
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

        readonly property int columns: Math.max(1, Math.floor((width - 56 + 18) / (250 + 18)))
        readonly property real cardWidth: (width - 56 - 18 * (columns - 1)) / columns

        // Grid, List and Headlines: one Flow.
        Flow {
            id: list
            visible: root.view !== "masonry"
            x: 28; y: 6
            width: flick.width - 56
            spacing: root.view === "grid" ? 18 : 0
            Accessible.role: Accessible.List
            Accessible.name: p.t("Links")
            Repeater {
                model: root.view !== "masonry" ? root.cards : []
                delegate: Loader {
                    required property var modelData
                    width: root.view === "grid" ? flick.cardWidth : list.width
                    sourceComponent: root.view === "grid" ? gridCard : (root.view === "list" ? listRow : headline)
                    property var card: modelData
                }
            }
        }

        // Masonry: each picture whole, in columns.
        Row {
            id: masonry
            visible: root.view === "masonry"
            x: 28; y: 6
            spacing: 18
            height: childrenRect.height
            Accessible.role: Accessible.List
            Accessible.name: p.t("Links")
            Repeater {
                model: root.view === "masonry" ? flick.columns : 0
                delegate: Column {
                    required property int index
                    readonly property int column: index
                    width: flick.cardWidth
                    spacing: 18
                    Repeater {
                        model: root.cards.filter((c, i) => i % flick.columns === parent.column)
                        delegate: Loader {
                            required property var modelData
                            width: flick.cardWidth
                            sourceComponent: gridCard
                            property var card: modelData
                            property bool whole: true
                        }
                    }
                }
            }
        }
    }

    // --- pieces ---------------------------------------------------------------------
    // The picture, or a placeholder in a colour from the address.
    component Picture: Item {
        id: pic
        property var card
        property bool whole: false
        // Height over width of the picture, once loaded.
        readonly property real ratio: image.implicitWidth > 0 ? image.implicitHeight / image.implicitWidth : 0.6
        clip: true
        Rectangle {
            anchors.fill: parent
            visible: image.status !== Image.Ready
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0; color: Theme.placeholder(pic.card.hue, 0) }
                GradientStop { position: 1; color: Theme.placeholder(pic.card.hue + 40, 1) }
            }
            Text { anchors.centerIn: parent; text: (pic.card.site || "?").charAt(0).toUpperCase()
                   color: Theme.accentInk; font.pixelSize: Math.min(parent.height * 0.4, 56); font.weight: Font.Bold }
        }
        Image {
            id: image
            anchors.fill: parent
            source: pic.card.image || ""
            fillMode: pic.whole ? Image.PreserveAspectFit : Image.PreserveAspectCrop
            asynchronous: true
            sourceSize.width: 520
        }
        BusyIndicator { anchors.centerIn: parent; running: !!pic.card.fetching; visible: running; width: 32; height: 32 }
    }

    // Favourite, Edit, Remove: on every card, shown on hover.
    component Actions: Item {
        id: acts
        property var card
        property bool shown: true
        property bool backdrop: false
        readonly property string what: card.title || card.url
        implicitWidth: buttons.implicitWidth + 6
        implicitHeight: 34
        width: implicitWidth; height: implicitHeight
        opacity: shown ? 1 : 0
        Rectangle { anchors.fill: parent; radius: 10; visible: acts.backdrop; color: Theme.alpha(Theme.card, 0.92) }
        Row {
            id: buttons
            anchors.centerIn: parent
            spacing: 2
            IconButton { width: 28; height: 28; iconSize: 14; flat: true; icon: "star"; checked: !!acts.card.favourite
                         label: (acts.card.favourite ? p.t("Unfavourite ") : p.t("Favourite ")) + acts.what
                         onClicked: p.activate("win.bookmark-favourite", String(acts.card.id)) }
            IconButton { width: 28; height: 28; iconSize: 14; flat: true; icon: "copy"
                         label: p.t("Copy link ") + acts.what
                         onClicked: p.activate("win.bookmark-copy", String(acts.card.id)) }
            IconButton { width: 28; height: 28; iconSize: 14; flat: true; icon: "pencil"
                         label: p.t("Edit ") + acts.what
                         onClicked: p.activate("win.bookmark-edit-ask", String(acts.card.id)) }
        }
    }

    component SiteLine: Row {
        property var card
        spacing: 6
        Rectangle {
            width: 16; height: 16; radius: 4
            color: Theme.fromOklch(0.52, 0.12, parent.card.hue * Math.PI / 180)
            anchors.verticalCenter: parent.verticalCenter
            Text { anchors.centerIn: parent; text: (parent.parent.card.site || "?").charAt(0).toUpperCase()
                   color: Theme.accentInk; font.pixelSize: 10; font.weight: Font.Bold }
        }
        Text { text: parent.card.site || ""; color: Theme.muted; font.pixelSize: 12
               anchors.verticalCenter: parent.verticalCenter }
    }

    component Tags: Row {
        property var card
        spacing: 6
        Repeater {
            model: (parent.card.tags || []).slice(0, 2)
            delegate: Rectangle {
                required property string modelData
                height: 20; radius: 10; width: t.implicitWidth + 14
                color: Theme.alpha(Theme.accent, 0.12)
                Text { id: t; anchors.centerIn: parent; text: parent.modelData; color: Theme.accentText; font.pixelSize: 11 }
            }
        }
    }

    Component {
        id: gridCard
        Rectangle {
            id: c
            readonly property var card: parent.card
            readonly property bool whole: !!parent.whole
            height: pic.height + info.implicitHeight + 26
            radius: 16
            color: Theme.card
            border.color: hover.hovered ? Theme.alpha(Theme.accent, 0.5) : Theme.line
            Accessible.role: Accessible.ListItem
            Accessible.name: card.title || card.url
            Accessible.description: card.site
            Accessible.onPressAction: p.activate("win.bookmark-open", String(card.id))
            Picture {
                id: pic
                x: 8; y: 8
                width: parent.width - 16
                card: c.card
                whole: c.whole
                height: c.whole && c.card.image ? Math.max(80, width * ratio) : 150
            }
            Column {
                id: info
                x: 14; anchors.top: pic.bottom; anchors.topMargin: 10
                width: parent.width - 28
                spacing: 6
                SiteLine { card: c.card }
                Text { width: parent.width; text: c.card.title || c.card.url; wrapMode: Text.Wrap
                       maximumLineCount: 2; elide: Text.ElideRight
                       color: Theme.ink; font.pixelSize: 15; font.weight: Font.DemiBold }
                Item {
                    width: parent.width; height: 22
                    Tags { card: c.card; anchors.verticalCenter: parent.verticalCenter }
                    Text { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                           text: c.card.date; color: Theme.faint; font.pixelSize: 12 }
                }
            }
            Actions {
                card: c.card
                backdrop: true
                shown: hover.hovered
                anchors.right: parent.right; anchors.rightMargin: 14; y: 14
            }
            // A favourite shows its star at all times.
            Rectangle {
                visible: !!c.card.favourite && !hover.hovered
                anchors.right: parent.right; anchors.rightMargin: 16; y: 16
                width: 28; height: 28; radius: 14; color: Theme.alpha(Theme.card, 0.92)
                Icon { anchors.centerIn: parent; name: "star"; size: 14; color: Theme.accentText }
            }
            HoverHandler { id: hover; cursorShape: Qt.PointingHandCursor }
            TapHandler { onTapped: p.activate("win.bookmark-open", String(c.card.id)) }
        }
    }

    Component {
        id: listRow
        Rectangle {
            id: r
            readonly property var card: parent.card
            height: 72
            color: hover.hovered ? Theme.hover : "transparent"
            radius: 12
            Accessible.role: Accessible.ListItem
            Accessible.name: card.title || card.url
            Accessible.description: card.site
            Accessible.onPressAction: p.activate("win.bookmark-open", String(card.id))
            Picture { id: thumb; x: 8; anchors.verticalCenter: parent.verticalCenter; width: 92; height: 56; card: r.card }
            Column {
                x: 116; anchors.verticalCenter: parent.verticalCenter
                width: parent.width - x - 240
                spacing: 4
                Text { width: parent.width; text: r.card.title || r.card.url; elide: Text.ElideRight
                       color: Theme.ink; font.pixelSize: 15; font.weight: Font.DemiBold }
                SiteLine { card: r.card }
            }
            Tags { card: r.card; anchors.right: parent.right; anchors.rightMargin: 200; anchors.verticalCenter: parent.verticalCenter }
            Text { anchors.right: parent.right; anchors.rightMargin: 110; anchors.verticalCenter: parent.verticalCenter
                   text: r.card.date; color: Theme.faint; font.pixelSize: 12 }
            Actions { card: r.card; anchors.right: parent.right; anchors.rightMargin: 8; anchors.verticalCenter: parent.verticalCenter }
            Rectangle { anchors.bottom: parent.bottom; x: 8; width: parent.width - 16; height: 1; color: Theme.line }
            HoverHandler { id: hover; cursorShape: Qt.PointingHandCursor }
            TapHandler { onTapped: p.activate("win.bookmark-open", String(r.card.id)) }
        }
    }

    Component {
        id: headline
        Rectangle {
            id: h
            readonly property var card: parent.card
            height: 54
            color: hover.hovered ? Theme.hover : "transparent"
            radius: 10
            Accessible.role: Accessible.ListItem
            Accessible.name: card.title || card.url
            Accessible.description: card.site
            Accessible.onPressAction: p.activate("win.bookmark-open", String(card.id))
            Text {
                x: 12; anchors.verticalCenter: parent.verticalCenter
                width: parent.width - 140
                elide: Text.ElideRight
                textFormat: Text.StyledText
                text: "<b>" + (h.card.title || h.card.url).replace(/&/g, "&amp;").replace(/</g, "&lt;") + "</b>"
                      + "&nbsp;&nbsp;<font color='" + Theme.faint + "'>" + h.card.site + " · " + h.card.date + "</font>"
                color: Theme.ink; font.pixelSize: 15
            }
            Actions { card: h.card; anchors.right: parent.right; anchors.rightMargin: 8; anchors.verticalCenter: parent.verticalCenter }
            Rectangle { anchors.bottom: parent.bottom; x: 8; width: parent.width - 16; height: 1; color: Theme.line }
            HoverHandler { id: hover; cursorShape: Qt.PointingHandCursor }
            TapHandler { onTapped: p.activate("win.bookmark-open", String(h.card.id)) }
        }
    }
}
