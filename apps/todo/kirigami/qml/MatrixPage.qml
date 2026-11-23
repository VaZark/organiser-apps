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
                            onClicked: page.openTask(modelData.id)
                        }
                    }
                }
            }
        }
    }
}
