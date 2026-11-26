import QtQuick
import QtQuick.Controls.Basic

// AI Agents (P7-T06; reference: ai-agents.png): usage over a period, the
// local servers with Open and Stop, and the projects set up for coding
// agents. Read on this computer; nothing is sent.
Rectangle {
    id: root
    property var model: ({ looking: true, servers: [], projects: [] })
    readonly property var usage: model.usage || ({ bars: [], legend: [] })
    color: Theme.pane
    Accessible.role: Accessible.Pane
    Accessible.name: p.t("AI Agents")

    Flickable {
        id: flick
        anchors.fill: parent
        contentHeight: page.implicitHeight + 40
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

        Column {
            id: page
            x: 28; y: 22
            width: flick.width - 56
            spacing: 18

            Row {
                width: parent.width
                spacing: 18
                // --- usage ---------------------------------------------------------
                Rectangle {
                    width: (parent.width - 18) * 0.56
                    height: 300
                    radius: 18; color: Theme.card; border.color: Theme.line
                    Accessible.role: Accessible.Grouping
                    Accessible.name: p.t("Usage")
                    Row {
                        x: 22; y: 20; spacing: 8
                        Icon { name: "chart"; size: 18; color: Theme.accentText; anchors.verticalCenter: parent.verticalCenter }
                        Text { text: p.t("Usage"); color: Theme.ink; font.pixelSize: 17; font.weight: Font.Bold
                               Accessible.role: Accessible.Heading; Accessible.name: text }
                    }
                    Segmented {
                        anchors.right: parent.right; anchors.rightMargin: 18; y: 14
                        name: p.t("Period")
                        options: [{ id: "today", label: p.t("Today") }, { id: "week", label: p.t("Week") },
                                  { id: "month", label: p.t("Month") }]
                        current: root.usage.period || "week"
                        onPicked: id => p.activate("win.agents-period", id)
                    }
                    Row {
                        x: 22; y: 66; spacing: 10
                        Text { id: big; text: root.usage.totalText || "0"; color: Theme.ink; font.pixelSize: 34; font.weight: Font.Bold
                               Accessible.role: Accessible.StaticText; Accessible.name: text + " " + (root.usage.caption || "") }
                        Text { text: root.usage.caption || ""; color: Theme.muted; font.pixelSize: 14
                               anchors.baseline: big.baseline }
                    }
                    // One bar per day.
                    Row {
                        id: bars
                        x: 22; y: 120
                        width: parent.width - 44; height: 120
                        spacing: Math.max(2, Math.min(10, width / Math.max(1, (root.usage.bars || []).length) * 0.25))
                        Repeater {
                            model: root.usage.bars || []
                            delegate: Item {
                                required property var modelData
                                width: (bars.width - bars.spacing * ((root.usage.bars || []).length - 1)) / Math.max(1, (root.usage.bars || []).length)
                                height: bars.height
                                Rectangle {
                                    anchors.bottom: parent.bottom
                                    width: parent.width; radius: 4
                                    height: Math.max(parent.modelData.value > 0 ? 4 : 2, parent.height * parent.modelData.share)
                                    color: parent.modelData.value > 0 ? Theme.chart : Theme.track
                                }
                            }
                        }
                    }
                    Row {
                        x: 22; y: 256; spacing: 16
                        Repeater {
                            model: root.usage.legend || []
                            delegate: Row {
                                required property var modelData
                                spacing: 6
                                Rectangle { width: 10; height: 10; radius: 3; color: Theme.chart; anchors.verticalCenter: parent.verticalCenter }
                                Text { text: parent.modelData.tool + " " + parent.modelData.text; color: Theme.muted; font.pixelSize: 13 }
                            }
                        }
                    }
                }
                // --- local servers ------------------------------------------------
                Rectangle {
                    width: (parent.width - 18) * 0.44
                    height: 300
                    radius: 18; color: Theme.card; border.color: Theme.line
                    Accessible.role: Accessible.Grouping
                    Accessible.name: p.t("On localhost")
                    Row {
                        x: 22; y: 20; spacing: 8
                        Icon { name: "globe"; size: 18; color: Theme.success; anchors.verticalCenter: parent.verticalCenter }
                        Text { text: p.t("On localhost"); color: Theme.ink; font.pixelSize: 17; font.weight: Font.Bold
                               Accessible.role: Accessible.Heading; Accessible.name: text }
                        Text { text: (root.model.servers || []).length
                                     + ((root.model.servers || []).length === 1 ? p.t(" server") : p.t(" servers"))
                               color: Theme.muted; font.pixelSize: 14
                               anchors.verticalCenter: parent.verticalCenter }
                    }
                    Text {
                        x: 22; y: 64
                        visible: (root.model.servers || []).length === 0
                        text: p.t("Nothing of yours is listening on this computer.")
                        color: Theme.faint; font.pixelSize: 13
                    }
                    ListView {
                        x: 12; y: 56
                        width: parent.width - 24; height: parent.height - 70
                        clip: true
                        model: root.model.servers || []
                        Accessible.role: Accessible.List
                        Accessible.name: p.t("Local servers")
                        delegate: Item {
                            id: server
                            required property var modelData
                            width: ListView.view.width; height: 52
                            Accessible.role: Accessible.ListItem
                            Accessible.name: ":" + modelData.port + " " + modelData.program
                            Rectangle { width: parent.width; height: 1; color: Theme.line }
                            Rectangle {
                                x: 10; anchors.verticalCenter: parent.verticalCenter
                                width: portText.implicitWidth + 18; height: 30; radius: 8
                                color: Theme.successFill
                                Text { id: portText; anchors.centerIn: parent; text: ":" + server.modelData.port
                                       color: Theme.successText; font.family: "monospace"; font.pixelSize: 14; font.weight: Font.Bold }
                            }
                            Text {
                                x: 100; anchors.verticalCenter: parent.verticalCenter
                                width: parent.width - 100 - 80; elide: Text.ElideRight
                                textFormat: Text.StyledText
                                text: "<b>" + server.modelData.program + "</b>"
                                      + (server.modelData.project ? " · " + server.modelData.project : "")
                                      + (server.modelData.uptime ? " · " + server.modelData.uptime : "")
                                color: Theme.ink; font.pixelSize: 14
                            }
                            Row {
                                anchors.right: parent.right; anchors.rightMargin: 6; anchors.verticalCenter: parent.verticalCenter
                                spacing: 4
                                IconButton { width: 30; height: 30; iconSize: 14; icon: "forward"
                                             label: p.t("Open in the browser: port ") + server.modelData.port
                                             onClicked: p.activate("win.agents-open", String(server.modelData.port)) }
                                IconButton { width: 30; height: 30; iconSize: 14; icon: "close"
                                             label: p.t("Stop the server on port ") + server.modelData.port
                                             onClicked: p.activate("win.agents-stop-ask", String(server.modelData.pid)) }
                            }
                        }
                    }
                }
            }

            // --- projects -----------------------------------------------------------
            Item {
                width: parent.width; height: 30
                Text { text: p.t("Projects"); color: Theme.ink; font.pixelSize: 18; font.weight: Font.Bold
                       anchors.verticalCenter: parent.verticalCenter
                       Accessible.role: Accessible.Heading; Accessible.name: text }
                PillButton { anchors.right: parent.right; implicitHeight: 30; text: p.t("Look Again")
                             onClicked: p.activate("win.agents-refresh", "") }
            }
            Text {
                visible: !root.model.looking && (root.model.projects || []).length === 0
                text: p.t("No projects are set up for coding agents yet.")
                color: Theme.faint; font.pixelSize: 14
            }
            Flow {
                width: parent.width
                spacing: 14
                Accessible.role: Accessible.List
                Accessible.name: p.t("Projects")
                Repeater {
                    model: root.model.projects || []
                    delegate: Rectangle {
                        id: project
                        required property var modelData
                        width: Math.max(300, (page.width - 28) / 3)
                        height: info.implicitHeight + 28
                        radius: 16; color: Theme.card; border.color: Theme.line
                        Accessible.role: Accessible.ListItem
                        Accessible.name: modelData.name
                        Accessible.description: modelData.tools.join(", ") + ", " + modelData.servers.length + p.t(" MCP servers")
                        Column {
                            id: info
                            x: 16; y: 14; width: parent.width - 32
                            spacing: 6
                            Row {
                                spacing: 10
                                MiniFolder { size: 30; kind: "code"; anchors.verticalCenter: parent.verticalCenter }
                                Column {
                                    Text { text: project.modelData.name; color: Theme.ink; font.pixelSize: 15; font.weight: Font.Bold }
                                    Text { text: project.modelData.where; color: Theme.faint; font.pixelSize: 12 }
                                }
                            }
                            Flow {
                                width: parent.width; spacing: 6
                                Repeater {
                                    model: project.modelData.tools
                                    delegate: Rectangle {
                                        required property string modelData
                                        height: 22; radius: 11; width: toolText.implicitWidth + 16
                                        color: Theme.alpha(Theme.accent, 0.12)
                                        Text { id: toolText; anchors.centerIn: parent; text: parent.modelData
                                               color: Theme.accentText; font.pixelSize: 11; font.weight: Font.DemiBold }
                                    }
                                }
                            }
                            Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.muted; font.pixelSize: 12
                                   text: project.modelData.files.join(" · ") }
                            Text {
                                visible: project.modelData.servers.length > 0
                                width: parent.width; wrapMode: Text.WordWrap; color: Theme.ink; font.pixelSize: 12
                                text: p.t("MCP: ") + project.modelData.servers.map(s => s.name).join(", ")
                            }
                            Text {
                                visible: !!project.modelData.tokensText
                                color: Theme.faint; font.pixelSize: 12
                                text: project.modelData.tokensText + (project.modelData.last ? " · " + project.modelData.last : "")
                            }
                            Row {
                                spacing: 6
                                PillButton { implicitHeight: 28; text: p.t("Open"); onClicked: p.activate("win.go-to", project.modelData.path) }
                                PillButton { implicitHeight: 28; text: p.t("Terminal"); onClicked: p.activate("win.open-terminal", project.modelData.path) }
                                PillButton { implicitHeight: 28; text: p.t("Editor"); onClicked: p.activate("win.open-editor", project.modelData.path) }
                            }
                        }
                    }
                }
            }
            Text { width: parent.width; wrapMode: Text.WordWrap; color: Theme.faint; font.pixelSize: 12
                   text: root.usage.note || "" }
        }
    }
}
