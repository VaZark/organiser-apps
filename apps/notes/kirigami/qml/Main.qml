import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.notes

Kirigami.ApplicationWindow {
    id: root
    title: i18n("Notes")
    width: Kirigami.Units.gridUnit * 60
    height: Kirigami.Units.gridUnit * 38
    visible: true

    Backend {
        id: notes
        Component.onCompleted: init()
    }

    readonly property var folderList: JSON.parse(notes.folders)
    readonly property var prefs: JSON.parse(notes.prefs)

    /// Make a page here, with an item as its parent, so Qt has no "not placed in the graphics
    /// scene" complaint; the page stack reparents it on push.
    function create(type, props) {
        const comp = Qt.createComponent(Qt.resolvedUrl(type + ".qml"))
        if (comp.status !== Component.Ready) {
            console.warn(comp.errorString())
            return null
        }
        return comp.createObject(pageStack, Object.assign({ backend: notes }, props || {}))
    }

    /// Show a scope in the first column; the details column closes.
    function showScope(name, folder) {
        notes.show(name, folder === undefined ? -1 : folder)
        while (pageStack.depth > 1) pageStack.pop()
    }

    /// The details of a note go in the column beside the list (a page of their own when narrow).
    function openNote(id) {
        notes.openNote(id)
        while (pageStack.depth > 1) pageStack.pop()
        const page = create("NoteDetailsPage", { noteId: id })
        if (page) pageStack.push(page)
    }

    /// A page for a note that does not exist yet; it is created by its Create action.
    function newNote() {
        while (pageStack.depth > 1) pageStack.pop()
        const page = create("NoteDetailsPage", { noteId: -1 })
        if (page) {
            page.created.connect(id => root.openNote(id))
            pageStack.push(page)
        }
    }

    /// Settings, tags and About open as a page beside the list (a page of their own when narrow),
    /// with Back to return. Not on Kirigami's layers: pushing there warns about its own anchors.
    function openBeside(page) {
        if (!page) return
        while (pageStack.depth > 1) pageStack.pop()
        pageStack.push(page)
    }

    function openFolderSettings(id) {
        openBeside(create("FolderSettingsPage", { folderId: id }))
    }

    function newFolder(parent) {
        const id = notes.createFolder(parent)
        if (id >= 0) {
            showScope("folder", id)
            openFolderSettings(id)
        }
    }

    function openSettings() {
        const page = create("SettingsPage")
        if (page) {
            // Tags open on top of the settings.
            page.openTags.connect(() => { const t = root.create("TagsPage"); if (t) pageStack.push(t) })
            root.openBeside(page)
        }
    }

    function openAbout() {
        const comp = Qt.createComponent(Qt.resolvedUrl("shared/AboutPage.qml"))
        const page = comp.createObject(pageStack, { appName: "Notes", appComment: "Capture notes as colourful cards", appId: "io.github.vazark.Notes" })
        openBeside(page)
    }

    /// The folders, one action each (indented by depth), made from the folder list.
    property var folderActions: []
    Instantiator {
        id: folderMaker
        model: root.folderList
        delegate: Kirigami.Action {
            required property var modelData
            text: " ".repeat(modelData.depth) + modelData.title
            icon.name: modelData.icon
            checkable: true
            checked: notes.scope === "folder" && notes.folderId === modelData.id
            onTriggered: root.showScope("folder", modelData.id)
        }
        onObjectAdded: Qt.callLater(root.collectFolders)
        onObjectRemoved: Qt.callLater(root.collectFolders)
    }
    function collectFolders() {
        const list = []
        for (let i = 0; i < folderMaker.count; ++i) list.push(folderMaker.objectAt(i))
        folderActions = list
    }

    readonly property Kirigami.Action appMenu: Kirigami.Action {
        icon.name: "application-menu"
        text: i18n("Menu")
        displayHint: Kirigami.DisplayHint.IconOnly
        Kirigami.Action {
            text: i18n("Configure Notes…"); icon.name: "configure"
            shortcut: StandardKey.Preferences
            onTriggered: root.openSettings()
        }
        Kirigami.Action {
            text: i18n("About Notes"); icon.name: "help-about"
            onTriggered: root.openAbout()
        }
        Kirigami.Action {
            text: i18n("Quit"); icon.name: "application-exit"
            shortcut: StandardKey.Quit
            onTriggered: Qt.quit()
        }
    }

    // The app menu sits at the right end of every page's toolbar, as in other KDE apps.
    Connections {
        target: root.pageStack
        function onPageInserted(position, page) {
            if (page && page.actions !== undefined && page.ownMenu !== true && !Array.from(page.actions).includes(root.appMenu))
                page.actions = [].concat(Array.from(page.actions), [root.appMenu])
        }
    }

    // One header across the whole window; the sidebar and the content sit side by side under it.
    pageStack.globalToolBar.style: Kirigami.ApplicationHeaderStyle.None
    header: AppHeader { window: root; iconOnly: true; sheets: true }
    // Too narrow for the list and the details side by side: stack them, one page at a time.
    pageStack.columnView.columnResizeMode: root.width < Kirigami.Units.gridUnit * 55
        ? Kirigami.ColumnView.SingleColumn : Kirigami.ColumnView.DynamicColumns

    // The sidebar lives inside the page row, so the header above it stays full width.
    pageStack.leftSidebar: root.globalDrawer

    // The sidebar: the scopes, then one entry per folder, then New Folder.
    Kirigami.Action {
        id: allAct
        text: i18n("All Notes")
        icon.name: "view-pim-notes"
        checkable: true
        checked: notes.scope === "notes"
        onTriggered: root.showScope("notes")
    }
    Kirigami.Action {
        id: favAct
        text: i18n("Favorites")
        icon.name: "emblem-favorite"
        checkable: true
        checked: notes.scope === "favorites"
        onTriggered: root.showScope("favorites")
    }
    Kirigami.Action {
        id: archiveAct
        text: i18n("Archive")
        icon.name: "folder-archive"
        checkable: true
        checked: notes.scope === "archive"
        onTriggered: root.showScope("archive")
    }
    Kirigami.Action {
        id: trashAct
        text: i18n("Trash")
        icon.name: "user-trash"
        checkable: true
        checked: notes.scope === "trash"
        onTriggered: root.showScope("trash")
    }
    Kirigami.Action {
        id: newFolderAct
        text: i18n("New Folder")
        icon.name: "folder-new"
        onTriggered: root.newFolder(-1)
    }
    Kirigami.Action { id: sidebarSeparator; separator: true }

    globalDrawer: Kirigami.GlobalDrawer {
        y: 0  // inside the page row, below the header already
        isMenu: false
        collapsible: false
        collapseButtonVisible: false
        // No handle sticking out of the window: in the narrow layout the header button opens the
        // sidebar and a click outside closes it. Docked (wide) it is simply open.
        handleVisible: false
        modal: Kirigami.Settings.isMobile || root.width < Kirigami.Units.gridUnit * 40
        drawerOpen: !modal
        onModalChanged: drawerOpen = !modal
        actions: [allAct, favAct, archiveAct, trashAct, sidebarSeparator].concat(root.folderActions, [newFolderAct])
    }

    pageStack.initialPage: NotesPage {
        backend: notes
        onOpenNote: id => root.openNote(id)
        onNewNote: root.newNote()
        onNewFolder: parent => root.newFolder(parent)
        onOpenFolderSettings: id => root.openFolderSettings(id)
        onShowScope: (name, folder) => root.showScope(name, folder)
        // The first page is set before the page stack signals insertions, so add the menu here.
        Component.onCompleted: if (!Array.from(actions).includes(root.appMenu)) actions = [].concat(Array.from(actions), [root.appMenu])
    }

    Connections {
        target: notes
        function onToastChanged() {
            if (notes.toast !== "") root.showPassiveNotification(notes.toast)
        }
    }

    // Development: `--screenshot FILE` saves the window as a PNG and quits. `--scope NAME`
    // (notes, favorites, archive, trash), `--folder ID`, `--open-note ID`, `--new-note`,
    // `--folder-settings ID`, `--page settings|tags|about|filter` choose what is shown.
    Timer {
        interval: 1200
        running: notes.arg("screenshot") !== ""
        onTriggered: {
            if (notes.arg("folder") !== "") root.showScope("folder", parseInt(notes.arg("folder")))
            else if (notes.arg("scope") !== "") root.showScope(notes.arg("scope"))
            if (notes.arg("open-note") !== "") root.openNote(parseInt(notes.arg("open-note")))
            if (notes.arg("new-note") !== "") root.newNote()
            if (notes.arg("folder-settings") !== "") root.openFolderSettings(parseInt(notes.arg("folder-settings")))
            const page = notes.arg("page")
            if (page === "settings") root.openSettings()
            else if (page === "tags") root.openBeside(root.create("TagsPage"))
            else if (page === "about") root.openAbout()
            grabTimer.start()
        }
    }
    Timer {
        id: grabTimer
        interval: 900
        onTriggered: root.header.parent.grabToImage(r => { r.saveToFile(notes.arg("screenshot")); Qt.quit() })
    }

    // `--new 1` (Sticky's "New in Notes"): a blank note, ready to write.
    Component.onCompleted: if (notes.arg("new") !== "") Qt.callLater(newNote)
}
