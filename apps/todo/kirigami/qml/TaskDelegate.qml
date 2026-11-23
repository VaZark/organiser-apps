import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.todo

/// One task row: a status checkbox, the title, and what else is worth knowing at a glance. Row
/// actions sit behind the "more" button (swipe on touch).
Kirigami.SwipeListItem {
    id: row
    required property var task
    property var statuses: []
    signal statusToggled(int id, bool closed)

    width: ListView.view ? ListView.view.width : implicitWidth
    signal pinToggled(int id, bool pinned)
    signal archiveToggled(int id, bool archived)
    signal removeRequested(int id)
    actions: [
        Kirigami.Action {
            text: row.task.pinned ? i18n("Unpin") : i18n("Pin")
            icon.name: "pin"
            onTriggered: row.pinToggled(row.task.id, !row.task.pinned)
        },
        Kirigami.Action {
            text: row.task.archived ? i18n("Restore") : i18n("Archive")
            icon.name: "folder-archive"
            onTriggered: row.archiveToggled(row.task.id, !row.task.archived)
        },
        Kirigami.Action {
            text: i18n("Delete…")
            icon.name: "edit-delete"
            onTriggered: row.removeRequested(row.task.id)
        }
    ]
    leftPadding: Kirigami.Units.largeSpacing + task.depth * Kirigami.Units.gridUnit * 1.5
    opacity: task.closed ? 0.6 : 1

    contentItem: RowLayout {
        spacing: Kirigami.Units.smallSpacing
        QQC2.CheckBox {
            checked: row.task.closed
            onToggled: row.statusToggled(row.task.id, checked)
        }
        StatusDot { tint: row.task.statusColor ? row.task.statusColor : Kirigami.Theme.disabledTextColor }
        ColumnLayout {
            spacing: 0
            Layout.fillWidth: true
            QQC2.Label {
                Layout.fillWidth: true
                text: row.task.title
                elide: Text.ElideRight
                font.strikeout: row.task.closed
                font.bold: row.task.pinned
            }
            QQC2.Label {
                visible: text !== ""
                Layout.fillWidth: true
                elide: Text.ElideRight
                font: Kirigami.Theme.smallFont
                color: Kirigami.Theme.disabledTextColor
                text: [row.task.epicTitle, row.task.due ? i18n("due %1", row.task.due) : ""].filter(s => s).join(" · ")
            }
        }
        Kirigami.Icon { visible: row.task.pinned; source: "pin"; Layout.preferredWidth: Kirigami.Units.iconSizes.small; Layout.preferredHeight: width }
        QQC2.Label {
            visible: row.task.overdue && !row.task.closed
            text: i18n("Overdue")
            color: Kirigami.Theme.negativeTextColor
            font: Kirigami.Theme.smallFont
        }
        Repeater {
            model: row.task.tags
            TagChip { required property string modelData; text: modelData }
        }
    }
}
