import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.sticky

/// The main window ("All notes"): every sticky as a card, and the settings. Each sticky that is
/// on the desktop has a window of its own (see StickyWindow). Everything shown comes from the
/// backend; nothing is decided here.
Kirigami.ApplicationWindow {
    id: root
    title: i18n("Sticky Notes")
    width: Kirigami.Units.gridUnit * 40
    height: Kirigami.Units.gridUnit * 30
    visible: false

    Backend {
        id: sticky
        Component.onCompleted: {
            init()
            if (opensManager()) root.openManager()
            root.checkExit()
        }
    }

    readonly property var desktopKeys: JSON.parse(sticky.desktopKeys)

    /// Show the main window, or bring the open one forward.
    function openManager() {
        show()
        raise()
        requestActivate()
    }

    /// End the process when nothing is left to show.
    function checkExit() {
        if (sticky.shouldExit(root.visible)) Qt.quit()
    }

    /// Bring a sticky's window forward; one that is only a card is put on the desktop first.
    function showSticky(key) {
        if (!root.desktopKeys.includes(key)) sticky.setDesktop(key, true)
        for (let i = 0; i < windows.count; ++i) {
            const w = windows.objectAt(i)
            if (w && w.key === key) {
                w.show()
                w.raise()
                w.requestActivate()
            }
        }
    }

    Instantiator {
        id: windows
        model: root.desktopKeys
        delegate: StickyWindow {
            required property string modelData
            key: modelData
            backend: sticky
            onShowAll: root.openManager()
            onHidden: Qt.callLater(root.checkExit)
        }
    }

    // Nothing is polled in the backend: this looks at a few files' modification times.
    Timer {
        interval: 700
        running: true
        repeat: true
        onTriggered: {
            sticky.poll()
            root.checkExit()
        }
    }

    onClosing: Qt.callLater(root.checkExit)

    /// Replace what the page stack shows with a page made from `component`. Made here, with an
    /// item as its parent, so Qt has no "not placed in the graphics scene" complaint.
    function pushPage(component) {
        pageStack.clear()
        const page = component.createObject(pageStack, { backend: sticky })
        if (page.showSticky) page.showSticky.connect(key => root.showSticky(key))
        pageStack.push(page)
    }

    function about() {
        const comp = Qt.createComponent(Qt.resolvedUrl("shared/AboutPage.qml"))
        pageStack.clear()
        pageStack.push(comp.createObject(pageStack, {
            appName: "Sticky Notes",
            appComment: "Keep notes, tasks and favourites on your desktop",
            appId: "io.github.vazark.Sticky"
        }))
    }

    Component {
        id: stickiesPage
        StickiesPage {}
    }
    Component {
        id: linkedAppsPage
        LinkedAppsPage {}
    }
    Component {
        id: coloursPage
        ColoursPage {}
    }
    Component {
        id: generalPage
        GeneralPage {}
    }

    readonly property Kirigami.Action appMenu: Kirigami.Action {
        icon.name: "application-menu"
        text: i18n("Menu")
        displayHint: Kirigami.DisplayHint.IconOnly
        Kirigami.Action {
            text: i18n("Configure Sticky Notes"); icon.name: "configure"
            Kirigami.Action {
                text: i18n("Linked Apps"); icon.name: "link"
                onTriggered: root.pushPage(linkedAppsPage)
            }
            Kirigami.Action {
                text: i18n("Colours"); icon.name: "preferences-desktop-color"
                onTriggered: root.pushPage(coloursPage)
            }
            Kirigami.Action {
                text: i18n("General"); icon.name: "preferences-system"
                onTriggered: root.pushPage(generalPage)
            }
        }
        Kirigami.Action {
            text: i18n("About Sticky Notes"); icon.name: "help-about"
            onTriggered: root.about()
        }
        Kirigami.Action { separator: true }
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
            if (page && page.actions !== undefined && !Array.from(page.actions).includes(root.appMenu))
                page.actions = [].concat(Array.from(page.actions), [root.appMenu])
        }
    }

    // One header across the whole window; the sidebar and the content sit side by side under it.
    pageStack.globalToolBar.style: Kirigami.ApplicationHeaderStyle.None
    header: AppHeader { window: root }
    // The sidebar lives inside the page row, so the header above it stays full width.
    pageStack.leftSidebar: root.globalDrawer

    globalDrawer: Kirigami.GlobalDrawer {
        y: 0  // inside the page row, below the header already
        isMenu: false
        collapsible: false
        collapseButtonVisible: false
        // No handle sticking out of the window: in the narrow layout the header button opens the
        // sidebar and a click outside closes it. Docked (wide) it is simply open.
        handleVisible: false
        modal: Kirigami.Settings.isMobile || root.width < Kirigami.Units.gridUnit * 30
        drawerOpen: !modal
        onModalChanged: drawerOpen = !modal
        actions: [
            Kirigami.Action {
                text: i18n("All Notes")
                icon.name: "view-list-details"
                onTriggered: root.pushPage(stickiesPage)
            }
        ]
    }

    Connections {
        target: sticky
        function onToastChanged() {
            if (sticky.toast !== "") root.showPassiveNotification(sticky.toast)
        }
    }

    Component.onCompleted: { pushPage(stickiesPage) }

    // Development: `--screenshot FILE` saves a window as a PNG and quits. `--page NAME` (linked,
    // colours, general, about) picks the page; `--shot sticky` grabs the first sticky's window.
    Timer {
        interval: 1200
        running: sticky.arg("screenshot") !== ""
        onTriggered: {
            const page = sticky.arg("page")
            if (page === "linked") root.pushPage(linkedAppsPage)
            else if (page === "colours") root.pushPage(coloursPage)
            else if (page === "general") root.pushPage(generalPage)
            else if (page === "about") root.about()
            grab.start()
        }
    }
    Timer {
        id: grab
        interval: 900
        onTriggered: {
            const target = sticky.arg("shot") === "sticky" && windows.count > 0
                ? windows.objectAt(0).sheetItem : root.pageStack
            target.grabToImage(r => { r.saveToFile(sticky.arg("screenshot")); Qt.quit() })
        }
    }
}
