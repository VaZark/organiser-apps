import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.kde.kirigami.delegates as KD
import org.organiser.todo

Kirigami.ApplicationWindow {
    id: root
    /// Dates as the system locale writes them; what is stored stays ISO.
    readonly property QtObject dates: LocaleDates {}
    title: "Todo"
    width: Kirigami.Units.gridUnit * 60
    height: Kirigami.Units.gridUnit * 38
    visible: true

    Backend {
        id: todo
        Component.onCompleted: init()
    }

    // Which page is showing: "list", "current", "board", "matrix", "archived", "unassigned".
    property string section: "list"
    property int selectedTask: -1
    readonly property var epicList: JSON.parse(todo.epics)

    function show(name, epic, noEpic) {
        section = name === "current" || name === "archived" ? (section === "notes" ? "notes" : "list") : name
        todo.scope = name === "current" ? "current" : name === "archived" ? "archived" : "all"
        todo.epic = epic === undefined ? -1 : epic
        todo.noEpic = noEpic === true
        todo.applyFilters()
        pushView(section === "list" || section === "notes" ? section : name)
    }

    /// Show the page of a view: "list", "notes", "board" or "matrix".
    function pushView(name) {
        if (name === "board") push("BoardPage")
        else if (name === "matrix") push("MatrixPage")
        else push("TasksPage", { backend: todo, notes: name === "notes" })
    }

    /// Replace what the page stack shows with the page of this type.
    function push(type, props) {
        pageStack.clear()
        // Made here, with an item as its parent, so Qt has no "not placed in the graphics scene"
        // complaint; the page stack reparents it on push.
        const comp = Qt.createComponent(Qt.resolvedUrl((type === "AboutPage" ? "shared/" : "") + type + ".qml"))
        const page = comp.createObject(pageStack, props || { backend: todo })
        pageStack.push(page)
        wire(page)
    }

    /// Open a page of Preferences on top of the Preferences page (the back button returns).
    function pushOn(type) {
        const comp = Qt.createComponent(Qt.resolvedUrl(type + ".qml"))
        const page = comp.createObject(pageStack, { backend: todo })
        pageStack.push(page)
        wire(page)
    }

    function wire(page) {
        // A page's own main action (Add) comes first; then the page-level menus; the app's menu last.
        page.actions = [].concat(page.primaryAction ? [page.primaryAction] : [], [viewMenu, filterAct, showMenu], Array.from(page.actions), [appMenu])
        if (page.openTask) page.openTask.connect(id => root.openTask(id))
        if (page.openEpicSettings) page.openEpicSettings.connect(id => root.openEpicSettings(id))
        if (page.openSection) page.openSection.connect(type => root.pushOn(type))
    }

    /// Show a task's details in the column beside the list (a page of its own on a narrow window).
    function openTask(id) {
        selectedTask = id
        while (pageStack.depth > 1) pageStack.pop()
        const comp = Qt.createComponent(Qt.resolvedUrl("TaskPage.qml"))
        const page = comp.createObject(pageStack, { backend: todo, taskId: id })
        page.openTask.connect(id => Qt.callLater(openTask, id))
        pageStack.push(page)
    }

    // The app menu sits at the right end of every page's toolbar, as in other KDE apps.
    readonly property Kirigami.Action appMenu: Kirigami.Action {
        icon.name: "application-menu"
        text: i18n("Menu")
        displayHint: Kirigami.DisplayHint.IconOnly
        Kirigami.Action {
            text: i18n("Configure Todo…"); icon.name: "configure"
            shortcut: StandardKey.Preferences
            onTriggered: root.push("PreferencesPage")
        }
        Kirigami.Action {
            text: i18n("About Todo"); icon.name: "help-about"
            onTriggered: root.push("AboutPage", { appName: "Todo", appComment: "Organise tasks, subtasks and deadlines", appId: "io.github.vazark.Todo" })
        }
        Kirigami.Action {
            text: i18n("Quit"); icon.name: "application-exit"
            shortcut: StandardKey.Quit
            onTriggered: Qt.quit()
        }
    }

    // Adding a task, from the list, the board and the matrix: a small dialog, `Buy milk #home due:tomorrow`.
    Kirigami.PromptDialog {
        id: addDialog
        title: i18n("Add Task")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        QQC2.TextField {
            id: addField
            placeholderText: i18n("Add a task… (#tag, due:tomorrow)")
            onAccepted: addDialog.accept()
        }
        onOpened: { addField.text = ""; addField.forceActiveFocus() }
        onAccepted: if (addField.text.trim() !== "") todo.createTask(addField.text, todo.epic, -1)
    }
    readonly property Kirigami.Action addAction: Kirigami.Action {
        text: i18n("Add Task")
        icon.name: "list-add"
        shortcut: StandardKey.New
        visible: todo.scope !== "archived"
        onTriggered: addDialog.open()
    }

    // What the lists include: a page-level menu (the Filter sheet has the rest), kept apart from the
    // app's own menu (Configure, About, Quit).
    readonly property Kirigami.Action showMenu: Kirigami.Action {
        text: i18n("Show")
        icon.name: "view-visible"
        displayHint: Kirigami.DisplayHint.IconOnly
        Kirigami.Action {
            text: i18n("Completed Tasks"); icon.name: "checkmark"
            checkable: true
            checked: todo.showClosed
            onTriggered: { todo.showClosed = !todo.showClosed; todo.applyFilters() }
        }
        Kirigami.Action {
            text: i18n("Subtasks"); icon.name: "view-list-tree"
            checkable: true
            checked: todo.showSubtasks
            onTriggered: { todo.showSubtasks = !todo.showSubtasks; todo.applyFilters() }
        }
        Kirigami.Action {
            text: i18n("Pinned Only"); icon.name: "pin"
            checkable: true
            checked: root.filterState.pinned === true
            onTriggered: todo.toggleFilter("pinned", root.filterState.pinned === true ? "0" : "1")
        }
        Kirigami.Action {
            text: i18n("Overdue Only"); icon.name: "appointment-missed"
            checkable: true
            checked: root.filterState.overdue === true
            onTriggered: todo.toggleFilter("overdue", root.filterState.overdue === true ? "0" : "1")
        }
    }

    // The views (how the tasks are laid out) live in one dropdown at the left of each page's toolbar;
    // Current, Unassigned and Archived are filters, so they live in the sidebar above the epics.
    readonly property Kirigami.Action viewMenu: Kirigami.Action {
        text: i18n("View")
        icon.name: "view-list-details"
        displayHint: Kirigami.DisplayHint.IconOnly
        Kirigami.Action {
            text: i18n("Tasks"); icon.name: "view-list-details"
            checkable: true
            checked: root.section === "list"
            onTriggered: root.setView("list")
        }
        Kirigami.Action {
            text: i18n("Notes"); icon.name: "view-list-icons"
            checkable: true
            checked: root.section === "notes"
            onTriggered: root.setView("notes")
        }
        Kirigami.Action {
            text: i18n("Board"); icon.name: "view-kanban"
            checkable: true
            checked: root.section === "board"
            onTriggered: root.setView("board")
        }
        Kirigami.Action {
            text: i18n("Matrix"); icon.name: "view-grid"
            checkable: true
            checked: root.section === "matrix"
            onTriggered: root.setView("matrix")
        }
    }

    // Search and every filter live in a sheet, as in Notes; the button shows when any is set.
    FilterSheet {
        id: filterSheet
        backend: todo
        onEpicPicked: (epic, noEpic) => root.setFilter(todo.scope, epic, noEpic)
    }
    readonly property var filterState: JSON.parse(todo.filters)
    readonly property Kirigami.Action filterAct: Kirigami.Action {
        text: root.filterState.count > 0 ? i18n("Filter (%1)", root.filterState.count) : i18n("Filter")
        icon.name: "view-filter"
        displayHint: Kirigami.DisplayHint.IconOnly
        checkable: true
        checked: root.filterState.count > 0
        shortcut: StandardKey.Find
        onTriggered: { checked = Qt.binding(() => root.filterState.count > 0); filterSheet.open() }
    }

    /// The epics, one action each, made from the epic list. Changing one opens its settings.
    property var epicActions: []
    Instantiator {
        id: epicMaker
        model: root.epicList
        delegate: Kirigami.Action {
            required property var modelData
            text: modelData.title
            icon.name: modelData.icon ? modelData.icon : "folder"
            checkable: true
            checked: todo.epic === modelData.id
            onTriggered: root.setFilter("all", modelData.id, false)
        }
        onObjectAdded: Qt.callLater(root.collectEpics)
        // Drop a removed action from the sidebar at once: the Instantiator destroys it right after
        // this signal, and a drawer item still pointing at it reads `icon` of null.
        onObjectRemoved: (index, object) => { root.epicActions = root.epicActions.filter(a => a !== object); Qt.callLater(root.collectEpics) }
    }
    function collectEpics() {
        const list = []
        for (let i = 0; i < epicMaker.count; ++i) { const a = epicMaker.objectAt(i); if (a) list.push(a) }
        epicActions = list
    }

    function openEpicSettings(id) {
        const comp = Qt.createComponent(Qt.resolvedUrl("EpicSettingsPage.qml"))
        const page = comp.createObject(pageStack, { backend: todo, epicId: id })
        if (!page) return
        // Beside the list, with Back (not on Kirigami's layers: pushing there warns about its anchors).
        while (pageStack.depth > 1) pageStack.pop()
        pageStack.push(page)
    }
    function newEpic() {
        const id = todo.createEpic(i18n("New Epic"))
        if (id >= 0) { setFilter("all", id, false); openEpicSettings(id) }
    }

    // Too narrow for the list and the details side by side: stack them, one page at a time.
    pageStack.columnView.columnResizeMode: root.width < Kirigami.Units.gridUnit * 70
        ? Kirigami.ColumnView.SingleColumn : Kirigami.ColumnView.DynamicColumns

    pageStack.defaultColumnWidth: Kirigami.Units.gridUnit * 30

    // The sidebar lists the epics; every view is under the toolbar's View menu.
    /// Change how the tasks are laid out (list, board, matrix); the filters stay as they are.
    function setView(name) {
        section = name
        pushView(name)
    }

    /// Change which tasks are shown, in whatever view is open: the list, board and matrix all read
    /// the same filtered tasks.
    function setFilter(scope, epic, noEpic) {
        todo.scope = scope
        todo.epic = epic
        todo.noEpic = noEpic
        todo.applyFilters()
        setView(section)
    }

    // The sidebar: the filters, then one entry per epic, then New Epic.
    Kirigami.Action {
        id: allAct
        text: i18n("All Tasks"); icon.name: "view-list-details"
        checkable: true
        checked: todo.scope === "all" && todo.epic < 0 && !todo.noEpic
        onTriggered: root.setFilter("all", -1, false)
    }
    Kirigami.Action {
        id: currentAct
        text: i18n("Current"); icon.name: "media-playback-start"
        checkable: true
        checked: todo.scope === "current"
        onTriggered: root.setFilter("current", -1, false)
    }
    Kirigami.Action {
        id: unassignedAct
        text: i18n("Unassigned (%1)", todo.unassignedOpen); icon.name: "folder-unknown"
        checkable: true
        checked: todo.noEpic
        onTriggered: root.setFilter("all", -1, true)
    }
    Kirigami.Action {
        id: archivedAct
        text: i18n("Archived"); icon.name: "folder-archive"
        checkable: true
        checked: todo.scope === "archived"
        onTriggered: root.setFilter("archived", -1, false)
    }
    Kirigami.Action {
        id: newEpicAct
        text: i18n("New Epic")
        icon.name: "folder-new"
        onTriggered: root.newEpic()
    }
    Kirigami.Action { id: sidebarSeparator; separator: true }

    // One header across the whole window; the sidebar and the content sit side by side under it.
    pageStack.globalToolBar.style: Kirigami.ApplicationHeaderStyle.None
    header: AppHeader { window: root; iconOnly: true; sheets: true }
    // The sidebar lives inside the page row, so the header above it stays full width.
    pageStack.leftSidebar: root.globalDrawer

    globalDrawer: Kirigami.GlobalDrawer {
        y: 0  // inside the page row, below the header already
        id: drawer
        isMenu: false
        collapsible: false
        collapseButtonVisible: false
        // No handle sticking out of the window: in the narrow layout the header button opens the
        // sidebar and a click outside closes it. Docked (wide) it is simply open.
        handleVisible: false
        modal: Kirigami.Settings.isMobile || root.width < Kirigami.Units.gridUnit * 40
        drawerOpen: !modal
        onModalChanged: drawerOpen = !modal
        // Fallback: never hand the drawer a null or destroyed action.
        actions: [allAct, currentAct, unassignedAct, archivedAct, sidebarSeparator].concat(root.epicActions, [newEpicAct]).filter(a => a)
    }

    Connections {
        target: todo
        function onToastChanged() {
            if (todo.toast !== "") root.showPassiveNotification(todo.toast)
        }
    }

    // Development: `--screenshot FILE` saves the window as a PNG and quits. `--page NAME` (list,
    // current, board, matrix, archived, unassigned, statuses, tags, prefs, about),
    // `--group status|epic|section`, `--open-task ID` and `--shot drawer|sheet|filter` choose what is shown and grabbed.
    Timer {
        interval: 1200
        running: todo.arg("screenshot") !== ""
        onTriggered: {
            const page = todo.arg("page")
            const pages = { statuses: "StatusesPage", tags: "TagsPage", prefs: "PreferencesPage", general: "GeneralPrefsPage" }
            if (pages[page]) root.push(pages[page])
            else if (page === "about") root.push("AboutPage", { appName: "Todo", appComment: "Organise tasks, subtasks and deadlines", appId: "io.github.vazark.Todo" })
            else if (page === "unassigned") root.show("list", -1, true)
            else if (page !== "") root.show(page)
            if (todo.arg("group") !== "") { todo.groupBy = todo.arg("group"); todo.applyFilters() }
            if (todo.arg("open-task") !== "") root.openTask(parseInt(todo.arg("open-task")))
            if (todo.arg("shot") === "drawer") globalDrawer.open()
            if (todo.arg("shot") === "filter") filterSheet.open()
            grabTimer.start()
        }
    }
    Timer {
        id: grabTimer
        interval: 900
        onTriggered: {
            const shot = todo.arg("shot")
            const item = shot === "drawer" ? globalDrawer.background.parent : shot === "filter" ? filterSheet.background.parent : root.header.parent
            item.grabToImage(r => { r.saveToFile(todo.arg("screenshot")); Qt.quit() })
        }
    }

    // `--new 1` (Sticky's "New in Todo"): ready to type a task.
    function startNew() {
        const page = pageStack.currentItem
        if (page && page.focusQuickAdd) page.focusQuickAdd()
    }

    Component.onCompleted: { if (todo.arg("new") !== "") Qt.callLater(startNew); show(todo.startView === "board" ? "board" : todo.startView === "quadrants" ? "matrix" : todo.startView === "notes" ? "notes" : "list") }
}
