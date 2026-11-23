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


    // ---- adding a task -----------------------------------------------------------------------
    /// Start typing a new task (`--new`, as Sticky's "New in Todo" opens the app).
    function focusQuickAdd() { applicationWindow().addAction.trigger() }

    // ---- the page's actions ------------------------------------------------------------------
    /// Narrow: this page's own actions are in the bottom bar and the top keeps only the app's menu.
    readonly property bool narrow: Kirigami.Settings.isMobile || applicationWindow().width < Kirigami.Units.gridUnit * 30
    readonly property bool pageBar: true
    /// What Main puts first in the top bar (see `wire`).
    readonly property var primaryAction: applicationWindow().addAction

    Kirigami.Action {
        id: configureEpicAction
        text: i18n("Configure Epic…")
        icon.name: "configure"
        visible: page.backend.epic >= 0
        onTriggered: page.openEpicSettings(page.backend.epic)
    }
    Kirigami.Action {
        id: groupAction
        text: i18n("Group By")
        icon.name: "view-group"
        displayHint: Kirigami.DisplayHint.IconOnly
            Kirigami.Action { text: i18n("None"); checkable: true; checked: page.activeGroup === ""; onTriggered: page.setGroup("") }
            Kirigami.Action { text: i18n("Status"); checkable: true; checked: page.activeGroup === "status"; onTriggered: page.setGroup("status") }
            // Epics split the tasks of all epics; sections split the tasks of one epic.
            Kirigami.Action { text: i18n("Epic"); visible: page.inEpic < 0 && !page.backend.noEpic; checkable: true; checked: page.activeGroup === "epic"; onTriggered: page.setGroup("epic") }
            Kirigami.Action { text: i18n("Section"); visible: page.inEpic >= 0; checkable: true; checked: page.activeGroup === "section"; onTriggered: page.setGroup("section") }
    }
    Kirigami.Action {
        id: sortAction
        text: i18n("Sort")
        icon.name: "view-sort"
        displayHint: Kirigami.DisplayHint.IconOnly
            Kirigami.Action { text: i18n("Manual"); onTriggered: { page.backend.sort = "manual"; page.backend.applyFilters() } }
            Kirigami.Action { text: i18n("Priority"); onTriggered: { page.backend.sort = "priority"; page.backend.applyFilters() } }
            Kirigami.Action { text: i18n("Newest"); onTriggered: { page.backend.sort = "created"; page.backend.applyFilters() } }
            Kirigami.Action { text: i18n("Due date"); onTriggered: { page.backend.sort = "due"; page.backend.applyFilters() } }
    }

    actions: [configureEpicAction, groupAction, sortAction]

    // The foot, narrow only, above any bulk-selection bar: up to three icons (add, view, filter) and
    // the page's own menu (what to show, grouping, sorting). The app's menu is at the top.
    footer: ColumnLayout {
        spacing: 0
        // The bulk actions, while any task is ticked.
        QQC2.ToolBar {
            Layout.fillWidth: true
            visible: page.pickedIds.length > 0
            contentItem: RowLayout {
                QQC2.ToolButton { icon.name: "dialog-cancel"; text: i18n("Clear"); display: QQC2.AbstractButton.IconOnly; onClicked: page.picked = ({}); QQC2.ToolTip.text: text; QQC2.ToolTip.visible: hovered }
                QQC2.Label { text: i18n("%1 selected", page.pickedIds.length); Layout.fillWidth: true; elide: Text.ElideRight }
                QQC2.ToolButton { text: i18n("Select All"); icon.name: "edit-select-all"; onClicked: page.pickAll() }
                QQC2.ToolButton {
                    text: i18n("Actions")
                    icon.name: "overflow-menu"
                    onClicked: bulkSheet.showActions(i18n("%1 selected", page.pickedIds.length), page.bulkActions)
                }
            }
        }
        PageBar {
            Layout.fillWidth: true
            visible: page.narrow
            icons: [applicationWindow().addAction, applicationWindow().viewMenu, applicationWindow().filterAct]
            menu: [applicationWindow().showMenu, groupAction, sortAction, configureEpicAction]
            heading: page.title
        }
    }

    // ---- selection for bulk actions ----------------------------------------------------------
    ActionDrawer { id: bulkSheet }
    readonly property list<Kirigami.Action> bulkActions: [
        Kirigami.Action { text: i18n("Finish"); icon.name: "checkmark"; onTriggered: page.bulkStatus(true) },
        Kirigami.Action { text: i18n("Reopen"); icon.name: "view-refresh"; onTriggered: page.bulkStatus(false) },
        Kirigami.Action { text: i18n("Pin"); icon.name: "pin"; onTriggered: page.bulk(id => page.backend.setPinned(id, true)) },
        Kirigami.Action {
            text: page.backend.scope === "archived" ? i18n("Restore") : i18n("Archive")
            icon.name: "archive-insert"
            onTriggered: page.bulk(id => page.backend.setArchived(id, page.backend.scope !== "archived"))
        },
        Kirigami.Action { text: i18n("Delete…"); icon.name: "edit-delete"; onTriggered: confirmDelete.open() }
    ]

    /// Ticked task ids (as keys); only those still in the list count, see `pickedIds`.
    property var picked: ({})
    readonly property var pickedIds: tasks.filter(t => !t.header && page.picked[t.id] === true).map(t => t.id)
    function pick(id, on) {
        const next = Object.assign({}, picked)
        if (on) next[id] = true; else delete next[id]
        picked = next
    }
    function pickAll() {
        const next = {}
        for (const t of tasks) if (!t.header) next[t.id] = true
        picked = next
    }
    /// Run `f` on every ticked task, then clear the selection.
    function bulk(f) {
        const ids = pickedIds
        picked = ({})
        ids.forEach(f)
    }
    function bulkStatus(closed) {
        const target = statuses.find(s => closed ? s.closed : !s.closed)
        if (target) bulk(id => backend.setStatus(id, target.id))
    }
    // Leaving a scope or epic drops the selection.
    Connections {
        target: page.backend
        function onScopeChanged() { page.picked = ({}) }
        function onEpicChanged() { page.picked = ({}) }
    }
    Kirigami.PromptDialog {
        id: confirmDelete
        title: i18n("Delete tasks")
        subtitle: i18n("Delete %1 selected tasks?", page.pickedIds.length)
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: i18n("Delete")
                icon.name: "edit-delete"
                onTriggered: { page.bulk(id => page.backend.deleteTask(id)); confirmDelete.close() }
            }
        ]
    }

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
            required property int index
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
                    // Between two task rows only: not above the first, nor under a group's header.
                    divider: row.index > 0 && !!page.tasks[row.index - 1] && !page.tasks[row.index - 1].header
                    selected: page.picked[row.modelData.id] === true
                    onClicked: page.openTask(row.modelData.id)
                    onSelectionToggled: (id, on) => page.pick(id, on)
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
