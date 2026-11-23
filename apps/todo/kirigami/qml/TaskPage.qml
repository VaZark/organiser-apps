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
    title: task.title ? task.title : i18n("Task")
    // Re-read when the lists change (taskRev), so an edit shows.
    readonly property var task: (taskRev, taskId >= 0 ? JSON.parse(backend.task(taskId)) : ({}))
    readonly property var statuses: JSON.parse(backend.statuses)
    readonly property var epics: JSON.parse(backend.epics)
    Connections { target: sheet.backend; function onTasksChanged() { sheet.taskRev++ } }
    property int taskRev: 0

    actions: [
        Kirigami.Action {
            text: sheet.task.pinned ? i18n("Unpin") : i18n("Pin")
            icon.name: "pin"
            onTriggered: sheet.backend.setPinned(sheet.taskId, !sheet.task.pinned)
        },
        Kirigami.Action {
            text: i18n("Add Subtask…")
            icon.name: "list-add"
            onTriggered: subtask.open()
        },
        Kirigami.Action {
            text: sheet.task.archived ? i18n("Restore") : i18n("Archive")
            icon.name: "folder-archive"
            onTriggered: { sheet.backend.setArchived(sheet.taskId, !sheet.task.archived); sheet.close() }
        },
        Kirigami.Action {
            text: i18n("Delete…")
            icon.name: "edit-delete"
            onTriggered: confirmDelete.open()
        }
    ]
    function close() { applicationWindow().pageStack.removePage(sheet) }

    Kirigami.FormLayout {
        wideMode: false
        width: sheet.width - Kirigami.Units.largeSpacing * 2
        x: Kirigami.Units.largeSpacing
        QQC2.TextField {
            id: titleField
            Kirigami.FormData.label: i18n("Title:")
            Layout.fillWidth: true
            text: sheet.task.title ? sheet.task.title : ""
            onEditingFinished: if (text !== sheet.task.title) sheet.backend.editTitle(sheet.taskId, text)
        }
        QQC2.ComboBox {
            Kirigami.FormData.label: i18n("Status:")
            model: sheet.statuses
            textRole: "name"
            valueRole: "id"
            currentIndex: sheet.statuses.findIndex(s => s.id === sheet.task.statusId)
            onActivated: sheet.backend.setStatus(sheet.taskId, currentValue)
        }
        QQC2.ComboBox {
            Kirigami.FormData.label: i18n("Priority:")
            model: ["highest", "high", "medium", "low", "lowest"]
            currentIndex: model.indexOf(sheet.task.priority)
            onActivated: sheet.backend.editPriority(sheet.taskId, currentText)
        }
        QQC2.ComboBox {
            Kirigami.FormData.label: i18n("Epic:")
            model: [{ id: -1, title: i18n("No epic") }].concat(sheet.epics)
            textRole: "title"
            valueRole: "id"
            currentIndex: Math.max(0, model.findIndex(e => e.id === (sheet.task.epic === null ? -1 : sheet.task.epic)))
            onActivated: sheet.backend.editEpic(sheet.taskId, currentValue)
        }
        QQC2.TextField {
            Kirigami.FormData.label: i18n("Due:")
            placeholderText: "YYYY-MM-DD"
            text: sheet.task.due ? sheet.task.due : ""
            onEditingFinished: sheet.backend.editDue(sheet.taskId, text)
        }
        QQC2.TextField {
            Kirigami.FormData.label: i18n("Tags:")
            placeholderText: i18n("comma, separated")
            text: sheet.task.tags ? sheet.task.tags.join(", ") : ""
            onEditingFinished: sheet.backend.setTaskTags(sheet.taskId, text)
        }
        MarkdownEditor {
            Kirigami.FormData.label: i18n("Notes:")
            Kirigami.FormData.labelAlignment: Qt.AlignTop
            Layout.fillWidth: true
            minimumHeight: Kirigami.Units.gridUnit * 14
            text: sheet.task.notes ? sheet.task.notes : ""
            onCommitted: t => { if (t !== sheet.task.notes) sheet.backend.editNotes(sheet.taskId, t) }
        }
    }

    Kirigami.PromptDialog {
        id: subtask
        title: i18n("New subtask")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        QQC2.TextField { id: subtaskTitle; placeholderText: i18n("Title") }
        onAccepted: if (subtaskTitle.text.trim() !== "") { sheet.backend.createTask(subtaskTitle.text, -1, sheet.taskId); subtaskTitle.text = "" }
    }
    Kirigami.PromptDialog {
        id: confirmDelete
        title: i18n("Delete this task?")
        subtitle: i18n("This cannot be undone. Archive it to keep it.")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: { sheet.backend.deleteTask(sheet.taskId); sheet.close() }
    }
}
