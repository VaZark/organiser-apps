import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.todo

Kirigami.ScrollablePage {
    id: page
    required property var backend
    signal openTask(int id)
    signal openEpicSettings(int id)
    /// The Notes view: the same tasks as cards that show their notes.
    property bool notes: false

    readonly property var statuses: JSON.parse(backend.statuses)
    title: backend.epic >= 0 ? JSON.parse(backend.epicInfo(backend.epic)).title : backend.scope === "archived" ? i18n("Archived")
         : backend.scope === "current" ? i18n("Current")
         : backend.noEpic ? i18n("Unassigned") : i18n("Tasks")


    // Quick add stays at the foot of the list: `Buy milk #home due:tomorrow`.
    footer: QQC2.ToolBar {
        visible: page.backend.scope !== "archived"
        contentItem: RowLayout {
            QQC2.TextField {
                id: quick
                Layout.fillWidth: true
                placeholderText: i18n("Add a task… (#tag, due:tomorrow)")
                onAccepted: if (text.trim() !== "") {
                    page.backend.createTask(text, page.backend.epic, -1)
                    text = ""
                }
            }
            QQC2.Button {
                text: i18n("Add")
                icon.name: "list-add"
                enabled: quick.text.trim() !== ""
                onClicked: quick.accepted()
            }
        }
    }

    actions: [
        Kirigami.Action {
            text: i18n("Configure Epic…")
            icon.name: "configure"
            visible: page.backend.epic >= 0
            onTriggered: page.openEpicSettings(page.backend.epic)
        },
        Kirigami.Action {
            text: i18n("Group By")
            icon.name: "view-group"
            Kirigami.Action { text: i18n("None"); checkable: true; checked: page.activeGroup === ""; onTriggered: page.setGroup("") }
            Kirigami.Action { text: i18n("Status"); checkable: true; checked: page.activeGroup === "status"; onTriggered: page.setGroup("status") }
            // Epics split the tasks of all epics; sections split the tasks of one epic.
            Kirigami.Action { text: i18n("Epic"); visible: page.inEpic < 0 && !page.backend.noEpic; checkable: true; checked: page.activeGroup === "epic"; onTriggered: page.setGroup("epic") }
            Kirigami.Action { text: i18n("Section"); visible: page.inEpic >= 0; checkable: true; checked: page.activeGroup === "section"; onTriggered: page.setGroup("section") }
        },
        Kirigami.Action {
            text: i18n("Sort")
            icon.name: "view-sort"
            Kirigami.Action { text: i18n("Manual"); onTriggered: { page.backend.sort = "manual"; page.backend.applyFilters() } }
            Kirigami.Action { text: i18n("Priority"); onTriggered: { page.backend.sort = "priority"; page.backend.applyFilters() } }
            Kirigami.Action { text: i18n("Newest"); onTriggered: { page.backend.sort = "created"; page.backend.applyFilters() } }
            Kirigami.Action { text: i18n("Due date"); onTriggered: { page.backend.sort = "due"; page.backend.applyFilters() } }
        }
    ]

    readonly property var tasks: JSON.parse(backend.tasks)
    readonly property int inEpic: backend.epic
    /// The grouping in effect: what was chosen, unless it does not fit the tasks shown.
    readonly property string activeGroup: {
        const g = backend.groupBy
        if (g === "status") return g
        if (g === "section" && inEpic >= 0) return g
        if (g === "epic" && inEpic < 0 && !backend.noEpic) return g
        return ""
    }

    // Notes: cards of a fixed width fill each row from the left; a row is one ListView item so
    // only the rows in view exist.
    readonly property real noteMargin: Kirigami.Units.gridUnit
    readonly property real noteGap: Kirigami.Units.largeSpacing
    readonly property real noteWidth: Math.min(Kirigami.Units.gridUnit * 22, Math.max(0, list.width - 2 * noteMargin))
    readonly property int noteCols: Math.max(1, Math.floor((list.width - 2 * noteMargin + noteGap) / (Kirigami.Units.gridUnit * 22 + noteGap)))
    readonly property var noteRows: {
        const out = []
        let cur = null
        for (const t of tasks) {
            if (t.header) { out.push(t); cur = null; continue }
            if (!cur || cur.cards.length >= noteCols) { cur = { cards: [] }; out.push(cur) }
            cur.cards.push(t)
        }
        return out
    }
    function setGroup(g) { backend.groupBy = g; backend.applyFilters() }

    Kirigami.PromptDialog {
        id: confirmDelete
        property int taskId: -1
        title: i18n("Delete this task?")
        subtitle: i18n("This cannot be undone. Archive it to keep it.")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: page.backend.deleteTask(taskId)
    }

    // One ListView is the page's flickable, so only the rows in view exist. Grouped lists carry
    // header rows (`header: true`) in the same model.
    ListView {
        id: list
        model: page.notes ? page.noteRows : page.tasks
        spacing: page.notes ? Kirigami.Units.largeSpacing : 0
        topMargin: page.notes ? Kirigami.Units.largeSpacing : 0
        bottomMargin: topMargin

        delegate: Loader {
            id: row
            required property var modelData
            width: ListView.view ? ListView.view.width : 0
            // Only while the ListView holds this row: Kirigami's SwipeListItem looks up through
            // its parents for the ListView when it is made, and fails on a row being released.
            active: ListView.view !== null && modelData !== undefined && modelData !== null
            sourceComponent: !modelData ? null : modelData.header ? headerComp : modelData.cards ? noteRowComp : taskComp

            Component {
                id: headerComp
                Kirigami.ListSectionHeader {
                    readonly property var group: row.modelData
                    label: group.title
                    contentItem: RowLayout {
                        spacing: Kirigami.Units.smallSpacing
                        StatusDot { visible: group.color !== null && group.color !== undefined; tint: group.color ? group.color : "transparent" }
                        Kirigami.Heading { level: 4; text: group.title; Layout.fillWidth: true; elide: Text.ElideRight }
                        QQC2.Label { text: group.count; color: Kirigami.Theme.disabledTextColor }
                    }
                }
            }
            Component {
                id: noteRowComp
                // One row of cards, top-aligned from the left edge, each at the usual card width.
                Row {
                    x: page.noteMargin
                    spacing: page.noteGap
                    Repeater {
                        model: row.modelData.cards
                        NoteCard {
                            required property var modelData
                            width: page.noteWidth
                            task: modelData
                            onClicked: page.openTask(modelData.id)
                        }
                    }
                }
            }
            Component {
                id: taskComp
                TaskDelegate {
                    task: row.modelData
                    onClicked: page.openTask(row.modelData.id)
                    onPinToggled: (id, pinned) => page.backend.setPinned(id, pinned)
                    onArchiveToggled: (id, archived) => page.backend.setArchived(id, archived)
                    onRemoveRequested: id => { confirmDelete.taskId = id; confirmDelete.open() }
                    onStatusToggled: (id, closed) => {
                        const target = closed
                            ? page.statuses.find(s => s.closed)
                            : page.statuses.find(s => !s.closed)
                        if (target) page.backend.setStatus(id, target.id)
                    }
                }
            }
        }

        Kirigami.PlaceholderMessage {
            anchors.centerIn: parent
            width: parent.width - Kirigami.Units.gridUnit * 4
            visible: list.count === 0
            icon.name: "checkmark"
            text: i18n("Nothing here")
            explanation: page.backend.scope === "archived" ? i18n("Archived tasks appear here.") : i18n("Type a task in the field below.")
        }
    }
}
