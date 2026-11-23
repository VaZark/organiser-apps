import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.todo

/// A task's details: the second column of the page stack on a wide window, a page of its own on a narrow one.
Kirigami.ScrollablePage {
    id: sheet
    required property var backend
    property int taskId: -1
    /// Open another task (a subtask, or this one's parent) in this column.
    signal openTask(int id)
    title: task.title ? task.title : i18n("Task")
    // Re-read when the lists change (taskRev), so an edit shows.
    readonly property var task: (taskRev, taskId >= 0 ? JSON.parse(backend.task(taskId)) : ({}))
    readonly property var statuses: JSON.parse(backend.statuses)
    readonly property var epics: JSON.parse(backend.epics)
    readonly property var subtasks: (taskRev, taskId >= 0 ? JSON.parse(backend.subtasks(taskId)) : [])
    readonly property int subtasksDone: subtasks.filter(t => t.closed).length
    readonly property var presets: JSON.parse(backend.colorPresets())
    Connections { target: sheet.backend; function onTasksChanged() { sheet.taskRev++ } }
    property int taskRev: 0

    // The task's colour tints the page, as a note's does.
    background: Rectangle {
        color: sheet.task.color ? Qt.alpha(sheet.task.color, 0.18) : Kirigami.Theme.backgroundColor
    }

    // Nothing in the header but Back: every action of the task is at the foot, in the bar or in
    // its menu, so there is one place to look and no second sheet to open from the top.
    readonly property bool roomy: width >= Kirigami.Units.gridUnit * 20
    Kirigami.Action {
        id: addSubtaskAction
        text: i18n("Add Subtask")
        icon.name: "list-add"
        onTriggered: subtaskField.forceActiveFocus()
    }
    Kirigami.Action {
        id: pinAction
        text: sheet.task.pinned ? i18n("Unpin") : i18n("Pin")
        icon.name: "pin"
        checkable: true
        checked: sheet.task.pinned === true
        onTriggered: sheet.backend.setPinned(sheet.taskId, !sheet.task.pinned)
    }
    Kirigami.Action {
        id: stickyAction
        text: sheet.task.sticky ? i18n("Remove Sticky Note") : i18n("Make Sticky Note")
        icon.name: "view-pim-notes"
        checkable: true
        checked: sheet.task.sticky === true
        onTriggered: sheet.backend.setSticky(sheet.taskId, !sheet.task.sticky)
    }

    // The rest of the foot's actions.
    Kirigami.Action {
        id: archiveAction
        text: sheet.task.archived ? i18n("Restore Task") : i18n("Archive Task")
        icon.name: "folder-archive"
        onTriggered: { sheet.backend.setArchived(sheet.taskId, !sheet.task.archived); sheet.close() }
    }
    Kirigami.Action {
        id: changeParentAction
        text: i18n("Change Parent…")
        icon.name: "go-up"
        onTriggered: { parentSearch.text = ""; parentDialog.open(); parentSearch.forceActiveFocus() }
    }
    Kirigami.Action {
        id: separateAction
        text: i18n("Make Separate Task")
        icon.name: "go-top"
        visible: !!sheet.task.parentTitle
        onTriggered: sheet.backend.setParent(sheet.taskId, -1)
    }
    Kirigami.Action {
        id: deleteAction
        text: i18n("Delete Task…")
        icon.name: "edit-delete"
        onTriggered: confirmDelete.open()
    }

    // The foot: up to three icons in one row while there is room (add a subtask, pin, sticky note),
    // and the menu for the rest. Without room the icons go into the menu too.
    footer: QQC2.ToolBar {
        position: QQC2.ToolBar.Footer
        contentItem: RowLayout {
            spacing: Kirigami.Units.smallSpacing
            Repeater {
                model: sheet.roomy ? [addSubtaskAction, pinAction, stickyAction] : []
                delegate: QQC2.ToolButton {
                    required property var modelData
                    icon.name: modelData.icon.name
                    text: modelData.text
                    display: QQC2.AbstractButton.IconOnly
                    checkable: modelData.checkable
                    checked: modelData.checked
                    QQC2.ToolTip.text: text
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    onClicked: modelData.trigger()
                }
            }
            Item { Layout.fillWidth: true }
            QQC2.ToolButton {
                id: taskMoreButton
                text: i18n("Task Actions")
                icon.name: "overflow-menu"
                display: QQC2.AbstractButton.IconOnly
                QQC2.ToolTip.text: text
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                // One plain list, no dividers: the icons first when there was no room for them in the
                // bar, then where the task is, then archive and delete.
                onClicked: taskSheet.showActions(i18n("Task"), (sheet.roomy ? [] : [addSubtaskAction, pinAction, stickyAction])
                    .concat([changeParentAction, separateAction, archiveAction, deleteAction]), taskMoreButton)
            }
        }
    }

    ActionDrawer { id: taskSheet }

    function close() { applicationWindow().pageStack.removePage(sheet) }

    // A field with its caption above it (Jira style), so two can share a row.
    component Field: ColumnLayout {
        id: field
        property string label
        default property alias content: slot.data
        spacing: Kirigami.Units.smallSpacing
        QQC2.Label {
            text: field.label
            font: Kirigami.Theme.smallFont
            color: Kirigami.Theme.disabledTextColor
        }
        ColumnLayout { id: slot; Layout.fillWidth: true; spacing: 0 }
    }

    // Wide: notes take the main column and the details sit beside them. Narrow: details first, then notes.
    GridLayout {
        id: grid
        readonly property bool wide: width >= Kirigami.Units.gridUnit * 40
        columns: wide ? 2 : 1
        columnSpacing: Kirigami.Units.gridUnit * 2
        rowSpacing: Kirigami.Units.largeSpacing
        width: sheet.width - Kirigami.Units.largeSpacing * 2
        x: Kirigami.Units.largeSpacing

        QQC2.TextField {
            id: titleField
            Layout.columnSpan: grid.columns
            Layout.fillWidth: true
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.4
            placeholderText: i18n("Title")
            text: sheet.task.title ? sheet.task.title : ""
            onEditingFinished: if (text !== sheet.task.title) sheet.backend.editTitle(sheet.taskId, text)
        }

        // Small text, only for a subtask: whose it is. Changing it or taking the task out is in the
        // menu at the foot (Change Parent…, Make Separate Task).
        QQC2.Label {
            Layout.row: 1
            Layout.column: 0
            Layout.columnSpan: grid.columns
            Layout.fillWidth: true
            visible: !!sheet.task.parentTitle
            elide: Text.ElideRight
            font: Kirigami.Theme.smallFont
            color: Kirigami.Theme.disabledTextColor
            text: i18n("Parent: %1", sheet.task.parentTitle ? sheet.task.parentTitle : "")
            HoverHandler { cursorShape: Qt.PointingHandCursor }
            TapHandler { onTapped: sheet.openTask(sheet.task.parent_id) }
        }

        ColumnLayout {
            Layout.row: grid.wide ? 2 : 3
            Layout.column: 0
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignTop
            spacing: Kirigami.Units.largeSpacing

            MarkdownEditor {
                Layout.fillWidth: true
                minimumHeight: grid.wide
                    ? Math.max(Kirigami.Units.gridUnit * 14, sheet.height - Kirigami.Units.gridUnit * 11 - subtasksBlock.height)
                    : Kirigami.Units.gridUnit * 14
                text: sheet.task.notes ? sheet.task.notes : ""
                onCommitted: t => { if (t !== sheet.task.notes) sheet.backend.editNotes(sheet.taskId, t) }
            }

            // The task's subtasks: how far along they are, each one to open or tick off, and a
            // line to add another.
            ColumnLayout {
                id: subtasksBlock
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing

                RowLayout {
                    Layout.fillWidth: true
                    Kirigami.Heading { level: 4; text: i18n("Subtasks") }
                    QQC2.Label {
                        visible: sheet.subtasks.length > 0
                        text: i18n("%1 of %2 done", sheet.subtasksDone, sheet.subtasks.length)
                        color: Kirigami.Theme.disabledTextColor
                    }
                    Item { Layout.fillWidth: true }
                }
                QQC2.ProgressBar {
                    visible: sheet.subtasks.length > 0
                    Layout.fillWidth: true
                    value: sheet.subtasks.length > 0 ? sheet.subtasksDone / sheet.subtasks.length : 0
                }
                Repeater {
                    model: sheet.subtasks
                    delegate: QQC2.ItemDelegate {
                        id: subRow
                        required property var modelData
                        Layout.fillWidth: true
                        onClicked: sheet.openTask(modelData.id)
                        contentItem: RowLayout {
                            spacing: Kirigami.Units.smallSpacing
                            QQC2.CheckBox {
                                checked: subRow.modelData.closed
                                onToggled: {
                                    const target = checked
                                        ? sheet.statuses.find(s => s.closed)
                                        : sheet.statuses.find(s => !s.closed)
                                    if (target) sheet.backend.setStatus(subRow.modelData.id, target.id)
                                }
                            }
                            StatusDot { tint: subRow.modelData.statusColor ? subRow.modelData.statusColor : Kirigami.Theme.disabledTextColor }
                            QQC2.Label {
                                Layout.fillWidth: true
                                text: subRow.modelData.title
                                elide: Text.ElideRight
                                font.strikeout: subRow.modelData.closed
                                opacity: subRow.modelData.closed ? 0.6 : 1
                            }
                            QQC2.Label {
                                visible: !!subRow.modelData.due && !subRow.modelData.closed
                                text: applicationWindow().dates.show(subRow.modelData.due)
                                color: subRow.modelData.overdue ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.disabledTextColor
                                font: Kirigami.Theme.smallFont
                            }
                        }
                    }
                }
                QQC2.Label {
                    visible: sheet.subtasks.length === 0
                    text: i18n("No subtasks yet.")
                    color: Kirigami.Theme.disabledTextColor
                }
                QQC2.TextField {
                    id: subtaskField
                    Layout.fillWidth: true
                    placeholderText: i18n("Add a subtask…")
                    onAccepted: if (text.trim() !== "") {
                        sheet.backend.createTask(text, -1, sheet.taskId)
                        text = ""
                    }
                }
            }
        }

        ColumnLayout {
            Layout.row: 2
            Layout.column: grid.wide ? 1 : 0
            Layout.alignment: Qt.AlignTop
            Layout.fillWidth: !grid.wide
            Layout.preferredWidth: grid.wide ? Kirigami.Units.gridUnit * 20 : -1
            spacing: Kirigami.Units.largeSpacing

            Kirigami.Heading { level: 4; text: i18n("Details") }

            RowLayout {
                Layout.fillWidth: true
                spacing: Kirigami.Units.largeSpacing
                Field {
                    label: i18n("Status")
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    QQC2.ComboBox {
                        Layout.fillWidth: true
                        model: sheet.statuses
                        textRole: "name"
                        valueRole: "id"
                        currentIndex: sheet.statuses.findIndex(s => s.id === sheet.task.statusId)
                        onActivated: sheet.backend.setStatus(sheet.taskId, currentValue)
                    }
                }
                Field {
                    label: i18n("Priority")
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    QQC2.ComboBox {
                        Layout.fillWidth: true
                        model: ["highest", "high", "medium", "low", "lowest"]
                        currentIndex: model.indexOf(sheet.task.priority)
                        onActivated: sheet.backend.editPriority(sheet.taskId, currentText)
                    }
                }
            }
            Field {
                label: i18n("Epic")
                Layout.fillWidth: true
                QQC2.ComboBox {
                    Layout.fillWidth: true
                    model: [{ id: -1, title: i18n("No epic") }].concat(sheet.epics)
                    textRole: "title"
                    valueRole: "id"
                    currentIndex: Math.max(0, model.findIndex(e => e.id === (sheet.task.epic === null ? -1 : sheet.task.epic)))
                    onActivated: sheet.backend.editEpic(sheet.taskId, currentValue)
                }
            }
            Field {
                label: i18n("Due date")
                Layout.fillWidth: true
                DateField {
                    Layout.fillWidth: true
                    text: sheet.task.due ? sheet.task.due : ""
                    onCommitted: iso => sheet.backend.editDue(sheet.taskId, iso)
                }
            }
            Field {
                label: i18n("Color")
                Layout.fillWidth: true
                ColorPicker {
                    Layout.fillWidth: true
                    presets: sheet.presets
                    color: sheet.task.color ? sheet.task.color : ""
                    onPicked: hex => sheet.backend.setTaskColor(sheet.taskId, hex)
                }
            }
            Field {
                label: i18n("Tags")
                Layout.fillWidth: true
                QQC2.TextField {
                    Layout.fillWidth: true
                    placeholderText: i18n("comma, separated")
                    text: sheet.task.tags ? sheet.task.tags.join(", ") : ""
                    onEditingFinished: sheet.backend.setTaskTags(sheet.taskId, text)
                }
            }
        }
    }

    // Pick the task this one goes under: open tasks except itself and its own subtasks.
    Kirigami.Dialog {
        id: parentDialog
        title: i18n("Change Parent")
        standardButtons: Kirigami.Dialog.Cancel
        padding: 0
        preferredWidth: Kirigami.Units.gridUnit * 24
        readonly property var choices: (sheet.taskRev, parentDialog.opened, JSON.parse(sheet.backend.parentChoices(sheet.taskId)))
            .filter(t => t.title.toLowerCase().includes(parentSearch.text.trim().toLowerCase()))
        ColumnLayout {
            spacing: 0
            Kirigami.SearchField {
                id: parentSearch
                Layout.fillWidth: true
                Layout.margins: Kirigami.Units.smallSpacing
            }
            QQC2.ScrollView {
                Layout.fillWidth: true
                Layout.preferredHeight: Kirigami.Units.gridUnit * 16
                ListView {
                    clip: true
                    model: parentDialog.choices
                    delegate: QQC2.ItemDelegate {
                        required property var modelData
                        width: ListView.view.width
                        text: modelData.epicTitle ? i18n("%1 · %2", modelData.title, modelData.epicTitle) : modelData.title
                        highlighted: modelData.id === sheet.task.parent_id
                        onClicked: { sheet.backend.setParent(sheet.taskId, modelData.id); parentDialog.close() }
                    }
                    QQC2.Label {
                        anchors.centerIn: parent
                        visible: parent.count === 0
                        text: i18n("No matching tasks")
                        color: Kirigami.Theme.disabledTextColor
                    }
                }
            }
        }
    }

    Kirigami.PromptDialog {
        id: confirmDelete
        title: i18n("Delete this task?")
        subtitle: i18n("This cannot be undone. Archive it to keep it.")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: { sheet.backend.deleteTask(sheet.taskId); sheet.close() }
    }
}
