import QtQuick

// A drop target (P2-T05): a folder card, a sidebar row, or the background
// (target "" means the folder on screen). Ctrl copies.
DropArea {
    property string target: ""
    keys: ["text/uri-list"]
    onDropped: drop => {
        if (!drop.hasUrls) return
        const uris = drop.urls.map(u => u.toString()).join("\n")
        // Qt proposes Copy while Ctrl is held.
        p.dropped(uris, target, drop.proposedAction === Qt.CopyAction)
        drop.acceptProposedAction()
    }
}
