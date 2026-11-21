import QtQuick
import QtQuick.Controls.Basic

// The Markdown page (P7-T07): Preview, Edit and Split, the outline of the
// headings, and Save and Revert. The preview is QML's own Markdown (no web
// engine, so no scripts); the text it gets has remote images removed.
Rectangle {
    id: root
    property var model: ({ name: "" })
    readonly property string mode: model.mode || "preview"
    readonly property bool showEditor: mode !== "preview"
    readonly property bool showPreview: mode !== "edit"
    color: Theme.pane
    Accessible.role: Accessible.Pane
    Accessible.name: p.t("Markdown")

    // The editor takes the text only when the file is (re)loaded, so typing
    // never moves the cursor.
    property int loaded: -1
    property bool loading: false
    function reload() {
        if (!visible || model.load === undefined || model.load === loaded) return
        loading = true
        editor.text = model.text || ""
        loading = false
        loaded = model.load
    }
    onModelChanged: reload()
    onVisibleChanged: reload()

    // Outline: jump to a heading.
    function jump(line) {
        if (showEditor) {
            const lines = editor.text.split("\n")
            let at = 0
            for (let i = 0; i < line && i < lines.length; i++) at += lines[i].length + 1
            editor.cursorPosition = at
            const r = editor.positionToRectangle(at)
            editorFlick.contentY = Math.max(0, Math.min(r.y - 12, editorFlick.contentHeight - editorFlick.height))
        }
        if (showPreview) {
            const share = line / Math.max(1, model.lines || 1)
            previewFlick.contentY = Math.max(0, Math.min(share * previewFlick.contentHeight - 12,
                                                         previewFlick.contentHeight - previewFlick.height))
        }
    }

    // --- toolbar --------------------------------------------------------------
    Item {
        id: bar
        width: parent.width; height: 60
        Segmented {
            x: 24; anchors.verticalCenter: parent.verticalCenter
            name: p.t("View")
            options: [{ id: "preview", label: p.t("Preview") }, { id: "edit", label: p.t("Edit") },
                      { id: "split", label: p.t("Split") }]
            current: root.mode
            onPicked: id => p.activate("win.md-mode", id)
        }
        Row {
            anchors.right: parent.right; anchors.rightMargin: 24; anchors.verticalCenter: parent.verticalCenter
            spacing: 10
            Text {
                visible: !!root.model.modified
                anchors.verticalCenter: parent.verticalCenter
                text: p.t("Unsaved changes")
                color: Theme.muted; font.pixelSize: 13
                Accessible.role: Accessible.StaticText; Accessible.name: text
            }
            PillButton { text: p.t("Revert"); enabled: !!root.model.modified; implicitHeight: 32
                         onClicked: p.activate("win.md-revert", "") }
            PillButton { text: p.t("Save"); primary: true; enabled: !!root.model.modified; implicitHeight: 32
                         onClicked: p.activate("win.md-save", "") }
        }
        Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.line }
    }

    // --- outline ----------------------------------------------------------------
    Rectangle {
        id: outline
        anchors.top: bar.bottom; anchors.bottom: parent.bottom
        width: (root.model.outline || []).length > 0 ? 230 : 0
        visible: width > 0
        color: Theme.sunken
        Text {
            x: 18; y: 16
            text: p.t("Outline"); color: Theme.muted
            font.pixelSize: 12; font.weight: Font.DemiBold; font.capitalization: Font.AllUppercase
        }
        ListView {
            x: 8; y: 42
            width: parent.width - 16; height: parent.height - 50
            clip: true
            model: root.model.outline || []
            Accessible.role: Accessible.List
            Accessible.name: p.t("Outline")
            delegate: Rectangle {
                required property var modelData
                width: ListView.view.width; height: 30
                radius: 8
                color: hover.hovered ? Theme.track : "transparent"
                Accessible.role: Accessible.ListItem
                Accessible.name: modelData.text
                Accessible.onPressAction: root.jump(modelData.line)
                Text {
                    x: 10 + (parent.modelData.level - 1) * 12
                    width: parent.width - x - 8
                    anchors.verticalCenter: parent.verticalCenter
                    text: parent.modelData.text; elide: Text.ElideRight
                    color: parent.modelData.level === 1 ? Theme.ink : Theme.muted
                    font.pixelSize: 13; font.weight: parent.modelData.level === 1 ? Font.DemiBold : Font.Normal
                }
                HoverHandler { id: hover }
                TapHandler { onTapped: root.jump(parent.modelData.line) }
            }
        }
    }

    // --- editor -------------------------------------------------------------------
    Item {
        id: body
        anchors.top: bar.bottom; anchors.bottom: parent.bottom
        anchors.left: outline.right; anchors.right: parent.right
        readonly property real half: root.showEditor && root.showPreview ? width / 2 : width

        Flickable {
            id: editorFlick
            visible: root.showEditor
            width: body.half; height: body.height
            clip: true
            contentHeight: editor.implicitHeight
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
            TextArea.flickable: TextArea {
                id: editor
                Accessible.name: p.t("Markdown text")
                font.family: "monospace"
                font.pixelSize: 14
                color: Theme.ink
                selectionColor: Theme.alpha(Theme.accent, 0.35)
                selectedTextColor: Theme.ink
                wrapMode: TextArea.Wrap
                padding: 24
                background: null
                onTextChanged: if (!root.loading) p.activate("win.md-edit", text)
            }
        }
        Rectangle {
            visible: root.showEditor && root.showPreview
            x: body.half; width: 1; height: body.height; color: Theme.line
        }

        // --- preview -------------------------------------------------------------
        Flickable {
            id: previewFlick
            visible: root.showPreview
            x: root.showEditor ? body.half + 1 : 0
            width: body.width - x; height: body.height
            clip: true
            contentHeight: preview.implicitHeight + 48
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
            Text {
                id: preview
                x: 32; y: 24
                width: Math.min(previewFlick.width - 64, 820)
                textFormat: Text.MarkdownText
                text: root.model.preview || ""
                baseUrl: root.model.base || ""
                wrapMode: Text.Wrap
                color: Theme.ink
                linkColor: Theme.accentText
                font.pixelSize: 15
                lineHeight: 1.25
                Accessible.role: Accessible.StaticText
                Accessible.name: p.t("Preview")
                onLinkActivated: link => p.activate("win.md-link", link)
                HoverHandler { cursorShape: preview.hoveredLink ? Qt.PointingHandCursor : Qt.ArrowCursor }
            }
        }
    }
}
