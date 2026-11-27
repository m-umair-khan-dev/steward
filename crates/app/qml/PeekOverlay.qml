import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Pdf
import QtMultimedia

// Peek (Space, P3-T11; reference: peek.png): images, PDFs (scroll, "Page n
// of m"), text and code, audio and video, with an Info column. Left and
// Right move through the folder; Escape or Space closes.
FocusScope {
    id: root
    property var model: ({ open: false })
    property bool info: true
    anchors.fill: parent
    z: 95
    visible: !!model.open

    Keys.onPressed: event => {
        if (event.key === Qt.Key_Escape || event.key === Qt.Key_Space) p.activate("win.peek-close", "")
        else if (event.key === Qt.Key_Right || event.key === Qt.Key_Down) p.activate("win.peek-move", "next")
        else if (event.key === Qt.Key_Left || event.key === Qt.Key_Up) p.activate("win.peek-move", "prev")
        else return
        event.accepted = true
    }

    Rectangle {
        anchors.fill: parent
        color: Theme.scrim(Theme.dark ? 0.6 : 0.35)
        MouseArea { anchors.fill: parent; onClicked: p.activate("win.peek-close", "") }
    }

    Rectangle {
        id: panel
        anchors.centerIn: parent
        width: Math.min(parent.width - 60, 1100)
        height: parent.height - 60
        radius: 18
        color: Theme.card
        border.color: Theme.line
        clip: true
        layer.enabled: Theme.effects
        layer.samplerName: "tex"
        layer.effect: FxShadow { shadowOpacity: 0.3; shadowBlur: 1.0; shadowVerticalOffset: 10 }
        Accessible.role: Accessible.Dialog
        Accessible.name: p.t("Peek: ") + (root.model.name || "")
        MouseArea { anchors.fill: parent }  // clicks inside do not close

        // Title bar.
        Item {
            id: bar
            width: parent.width
            height: 60
            Rectangle {
                x: 18; anchors.verticalCenter: parent.verticalCenter
                width: 30; height: 36; radius: 5
                color: Theme.badge(root.model.kind || "other")
                Text { anchors.centerIn: parent; text: (root.model.badge || "").slice(0, 4); color: Theme.accentInk
                       font.pixelSize: 9; font.weight: Font.Black }
            }
            Column {
                x: 60; anchors.verticalCenter: parent.verticalCenter
                width: parent.width - x - buttons.width - 24
                Text { width: parent.width; text: root.model.name || ""; elide: Text.ElideMiddle; color: Theme.ink
                       font.pixelSize: 15; font.weight: Font.Bold }
                Text { width: parent.width; text: root.model.meta || ""; color: Theme.muted; font.pixelSize: 12 }
            }
            Row {
                id: buttons
                anchors.right: parent.right; anchors.rightMargin: 14
                anchors.verticalCenter: parent.verticalCenter
                spacing: 8
                PillButton { text: p.t("Open"); primary: true; implicitHeight: 32; onClicked: p.activate("win.peek-open", "") }
                PillButton { text: p.t("Info"); implicitHeight: 32; onClicked: root.info = !root.info }
                IconButton { width: 32; height: 32; iconSize: 14; icon: "close"; label: p.t("Close")
                             onClicked: p.activate("win.peek-close", "") }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.line }
        }

        // The preview.
        Rectangle {
            id: stage
            anchors.top: bar.bottom
            anchors.bottom: footer.top
            anchors.left: parent.left
            anchors.right: side.left
            anchors.margins: 14
            radius: 12
            color: Theme.sunken
            clip: true

            Loader {
                anchors.fill: parent
                active: root.visible
                sourceComponent: {
                    switch (root.model.mode) {
                    case "image": return imageView
                    case "pdf": return pdfView
                    case "text": return textView
                    case "media": return mediaView
                    default: return otherView
                    }
                }
            }
        }

        // Info column.
        Rectangle {
            id: side
            anchors.top: bar.bottom
            anchors.bottom: footer.top
            anchors.right: parent.right
            width: root.info ? 270 : 0
            color: Theme.card
            clip: true
            Behavior on width { enabled: Theme.motion; NumberAnimation { duration: 180 } }
            Column {
                x: 18; y: 16
                width: parent.width - 36
                spacing: 8
                Text { text: p.t("Info"); color: Theme.ink; font.pixelSize: 14; font.weight: Font.Bold }
                Repeater {
                    model: root.model.facts || []
                    delegate: Item {
                        required property var modelData
                        width: parent.width
                        height: Math.max(k.height, v.height)
                        Text { id: k; text: parent.modelData[0]; color: Theme.faint; font.pixelSize: 12; width: 84 }
                        Text { id: v; x: 88; width: parent.width - 88; text: parent.modelData[1]; color: Theme.ink
                               font.pixelSize: 12; wrapMode: Text.WrapAnywhere
                               Accessible.role: Accessible.StaticText; Accessible.name: parent.modelData[0] + " " + text }
                    }
                }
            }
        }

        Item {
            id: footer
            anchors.bottom: parent.bottom
            width: parent.width
            height: 44
            Text {
                anchors.centerIn: parent
                text: (root.model.position || "") + p.t("    ← → to move · Space or Esc to close")
                color: Theme.muted; font.pixelSize: 12
            }
        }
    }

    Component {
        id: imageView
        Image {
            source: root.model.uri || ""
            fillMode: Image.PreserveAspectFit
            asynchronous: true
            anchors.margins: 12
        }
    }

    Component {
        id: pdfView
        Item {
            PdfDocument {
                id: doc
                source: root.model.uri || ""
                onStatusChanged: if (status === PdfDocument.Ready) p.peekpages(pageCount)
            }
            PdfMultiPageView {
                id: pages
                anchors.fill: parent
                document: doc
            }
            Rectangle {
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.bottom: parent.bottom; anchors.bottomMargin: 12
                width: pageLabel.implicitWidth + 24; height: 26; radius: 13
                color: Theme.caption
                visible: doc.pageCount > 0
                Text {
                    id: pageLabel
                    anchors.centerIn: parent
                    text: p.t("Page ") + (pages.currentPage + 1) + p.t(" of ") + doc.pageCount
                    color: Theme.accentInk; font.pixelSize: 12
                    Accessible.role: Accessible.StaticText
                    Accessible.name: text
                }
            }
        }
    }

    Component {
        id: textView
        ScrollView {
            TextArea {
                readOnly: true
                Accessible.name: p.t("Contents of ") + (root.model.name || "")
                text: root.model.text || ""
                font.family: "monospace"
                font.pixelSize: 13
                color: Theme.ink
                wrapMode: TextArea.NoWrap
                background: null
                padding: 16
            }
        }
    }

    Component {
        id: mediaView
        Item {
            MediaPlayer {
                id: player
                source: root.model.uri || ""
                audioOutput: AudioOutput { volume: 0.8 }
                videoOutput: video
            }
            VideoOutput { id: video; anchors.fill: parent }
            Icon {
                anchors.centerIn: parent
                visible: player.playbackState !== MediaPlayer.PlayingState
                name: root.model.kind === "audio" ? "music" : "videos"
                size: 64; color: Theme.faint
            }
            Row {
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.bottom: parent.bottom; anchors.bottomMargin: 12
                spacing: 8
                PillButton {
                    text: player.playbackState === MediaPlayer.PlayingState ? p.t("Pause") : p.t("Play")
                    onClicked: player.playbackState === MediaPlayer.PlayingState ? player.pause() : player.play()
                }
            }
        }
    }

    Component {
        id: otherView
        Item {
            Rectangle {
                anchors.centerIn: parent
                width: 120; height: 150; radius: 12
                color: Theme.badge(root.model.kind || "other")
                Text { anchors.centerIn: parent; text: root.model.badge || ""; color: Theme.accentInk
                       font.pixelSize: 22; font.weight: Font.Black }
            }
        }
    }
}
