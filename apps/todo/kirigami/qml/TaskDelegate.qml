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
    /// A hairline above the row: only between two rows, so the last one has none hanging below.
    property bool divider: false
    signal selectionToggled(int id, bool on)

    width: ListView.view ? ListView.view.width : implicitWidth
    // The task's colour is the stripe at the row's left edge.
    background: Item {
        Rectangle {
            anchors.fill: parent
            anchors.topMargin: 1
            anchors.bottomMargin: 1
            // Only the stripe carries the colour; the row itself is plain.
            color: row.selected ? Qt.alpha(Kirigami.Theme.highlightColor, 0.25)
                 : row.hovered ? Kirigami.Theme.hoverColor : "transparent"
            Rectangle { visible: !!row.task.effectiveColor; width: Kirigami.Units.smallSpacing; height: parent.height; color: row.task.effectiveColor ? row.task.effectiveColor : "transparent" }
        }
        Kirigami.Separator { visible: row.divider; anchors { left: parent.left; right: parent.right; top: parent.top } }
    }
    leftPadding: Kirigami.Units.largeSpacing + task.depth * Kirigami.Units.gridUnit * 1.5
    opacity: task.closed ? 0.6 : 1

    contentItem: RowLayout {
        spacing: Kirigami.Units.smallSpacing
        QQC2.CheckBox {
            checked: row.selected
            onToggled: row.selectionToggled(row.task.id, checked)
        }
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
        // The status, at the right edge.
        StatusDot { Layout.leftMargin: Kirigami.Units.smallSpacing; tint: row.task.statusColor ? row.task.statusColor : Kirigami.Theme.disabledTextColor }
    }
}
