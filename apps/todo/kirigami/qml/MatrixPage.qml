import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.todo

/// The Eisenhower matrix: open tasks in four boxes by importance and urgency.
Kirigami.Page {
    id: page
    required property var backend
    signal openTask(int id)
    title: i18n("Matrix")

    // ---- the page's actions ------------------------------------------------------------------
    /// Narrow: this page's own actions are in the bottom bar and the top keeps only the app's menu.
    readonly property bool narrow: Kirigami.Settings.isMobile || applicationWindow().width < Kirigami.Units.gridUnit * 30
    readonly property bool pageBar: true
    /// What Main puts first in the top bar (see `wire`).
    readonly property var primaryAction: applicationWindow().addAction

    // The foot, narrow only: up to three icons (add, view, filter) and the page's own menu (what to
    // show). The app's menu is at the top.
    footer: PageBar {
        visible: page.narrow
        icons: [applicationWindow().addAction, applicationWindow().viewMenu, applicationWindow().filterAct]
        menu: [applicationWindow().showMenu]
        heading: page.title
    }

    readonly property var tasks: JSON.parse(backend.allTasks).filter(t => !t.closed)
    readonly property var boxes: [
        { title: i18n("Do first"), hint: i18n("Important and urgent") },
        { title: i18n("Schedule"), hint: i18n("Important, not urgent") },
        { title: i18n("Delegate"), hint: i18n("Urgent, not important") },
        { title: i18n("Backlog"), hint: i18n("Neither") }
    ]

    GridLayout {
        anchors.fill: parent
        columns: page.width < Kirigami.Units.gridUnit * 40 ? 1 : 2
        Repeater {
            model: page.boxes
            delegate: Rectangle {
                id: box
                required property var modelData
                required property int index
                Layout.fillWidth: true
                Layout.fillHeight: true
                color: Kirigami.Theme.alternateBackgroundColor
                radius: Kirigami.Units.cornerRadius
                readonly property var items: page.tasks.filter(t => t.quadrant === index)
                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: Kirigami.Units.smallSpacing
                    Kirigami.Heading { level: 3; text: box.modelData.title + " (" + box.items.length + ")" }
                    QQC2.Label { text: box.modelData.hint; font: Kirigami.Theme.smallFont; color: Kirigami.Theme.disabledTextColor }
                    ListView {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        clip: true
                        model: box.items
                        delegate: QQC2.ItemDelegate {
                            required property var modelData
                            width: ListView.view.width
                            text: modelData.title
                            // Clear of the colour stripe: the label's own inset, which the delegate's
                            // padding does not reach.
                            contentItem: QQC2.Label {
                                text: parent.text
                                elide: Text.ElideRight
                                verticalAlignment: Text.AlignVCenter
                                leftPadding: Kirigami.Units.gridUnit * 0.75
                                rightPadding: Kirigami.Units.smallSpacing
                            }
                            // The task's own colour, else the nearest above it (as in the list).
                            background: Rectangle {
                                color: parent.modelData.effectiveColor ? Qt.alpha(parent.modelData.effectiveColor, parent.hovered ? 0.4 : 0.28) : (parent.hovered ? Kirigami.Theme.hoverColor : "transparent")
                                Rectangle {
                                    visible: !!parent.parent.modelData.effectiveColor
                                    width: Kirigami.Units.smallSpacing
                                    height: parent.height
                                    color: parent.parent.modelData.effectiveColor ? parent.parent.modelData.effectiveColor : "transparent"
                                }
                            }
                            onClicked: page.openTask(modelData.id)
                        }
                    }
                }
            }
        }
    }
}
