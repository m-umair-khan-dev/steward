import QtQuick

// "Stop this server?" (P7-T05): stopping sends a normal terminate signal,
// only after this question.
Sheet {
    id: root
    property var info: ({})
    title: p.t("Stop the Server?")
    cardWidth: 420
    Column {
        width: parent.width
        spacing: 14
        Text {
            width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: 14
            text: (root.info.program || "") + p.t(" on port ") + (root.info.port || "")
                  + (root.info.project ? " (" + root.info.project + ")" : "")
                  + p.t(" will be asked to stop. Unsaved work in it may be lost.")
        }
        Row {
            anchors.right: parent.right
            spacing: 8
            PillButton { text: p.t("Cancel"); onClicked: p.activate("win.agents-stop", "no") }
            PillButton { text: p.t("Stop"); destructive: true; onClicked: p.activate("win.agents-stop", "yes") }
        }
    }
}
