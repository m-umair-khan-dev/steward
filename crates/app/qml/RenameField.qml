import QtQuick
import QtQuick.Controls.Basic

// Edits an item's name in place (F2, P2-T03). Enter or leaving the field
// renames; Escape cancels. Files start with the name before the extension
// selected.
TextField {
    id: field
    property string uri: ""
    property string original: ""
    property bool isFolder: false
    property bool finished: false

    text: original
    horizontalAlignment: TextInput.AlignHCenter
    color: Theme.ink
    font.pixelSize: 13
    padding: 4
    selectByMouse: true
    Accessible.name: "New name"
    background: Rectangle { radius: 7; color: Theme.card; border.color: Theme.accent; border.width: 2 }

    function finish(name) {
        if (finished) return
        finished = true
        p.rename(uri, name)
    }
    Component.onCompleted: {
        const dot = original.lastIndexOf(".")
        select(0, !isFolder && dot > 0 ? dot : original.length)
        forceActiveFocus()
    }
    onAccepted: finish(text)
    Keys.onEscapePressed: finish("")
    onActiveFocusChanged: if (!activeFocus) finish(text)
}
