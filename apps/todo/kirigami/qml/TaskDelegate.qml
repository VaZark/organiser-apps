import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.todo

/// One task row: a selection checkbox (for the bulk actions of the list), the title, and what else is worth knowing at a glance (pinned,
/// sticky, overdue, tags). It has no buttons of its own: a click opens the task, whose page has
/// Pin, Sticky Note, Archive and Delete.
Kirigami.SwipeListItem {
    id: row
    required property var task
    property var statuses: []
    /// Ticked in the list's selection.
    property bool selected: false
    signal selectionToggled(int id, bool on)

    width: ListView.view ? ListView.view.width : implicitWidth
    // The task's own colour tints its row (the same rule as a note's card).
    background: Rectangle {
        color: row.selected ? Qt.alpha(Kirigami.Theme.highlightColor, 0.25)
             : row.task.color ? Qt.alpha(row.task.color, row.hovered ? 0.4 : 0.28) : (row.hovered ? Kirigami.Theme.hoverColor : "transparent")
        Rectangle { visible: !!row.task.color; width: Kirigami.Units.smallSpacing; height: parent.height; color: row.task.color ? row.task.color : "transparent" }
    }
    leftPadding: Kirigami.Units.largeSpacing + task.depth * Kirigami.Units.gridUnit * 1.5
    opacity: task.closed ? 0.6 : 1

    contentItem: RowLayout {
        spacing: Kirigami.Units.smallSpacing
        QQC2.CheckBox {
            checked: row.selected
            onToggled: row.selectionToggled(row.task.id, checked)
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
                text: [row.task.epicTitle, row.task.due ? i18n("due %1", applicationWindow().dates.show(row.task.due)) : ""].filter(s => s).join(" · ")
            }
        }
        Kirigami.Icon { visible: row.task.sticky === true; source: "view-pim-notes"; Layout.preferredWidth: Kirigami.Units.iconSizes.small; Layout.preferredHeight: width }
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
