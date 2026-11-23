import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.recipes

Kirigami.ApplicationWindow {
    id: root
    /// Dates as the system locale writes them; what is stored stays ISO.
    readonly property QtObject dates: LocaleDates {}
    title: i18n("Recipes")
    width: Kirigami.Units.gridUnit * 60
    height: Kirigami.Units.gridUnit * 38
    visible: true

    Backend {
        id: backend
        Component.onCompleted: init()
    }

    // The section showing in the first column: "recipes", "calendar", "fridge" or "shop".
    property string section: "recipes"
    readonly property var pages: ({
        recipes: "RecipesPage.qml",
        calendar: "CalendarPage.qml",
        fridge: "FridgePage.qml",
        shop: "ShopPage.qml"
    })

    pageStack.defaultColumnWidth: Kirigami.Units.gridUnit * 26

    /// A page made here, with an item as its parent, so Qt has no "not placed in the graphics
    /// scene" complaint; the page stack reparents it on push.
    function make(file, props) {
        const comp = Qt.createComponent(Qt.resolvedUrl(file))
        if (comp.status !== Component.Ready) {
            console.warn(comp.errorString())
            return null
        }
        return comp.createObject(pageStack, props)
    }

    function show(name) {
        section = name
        pageStack.layers.clear()
        pageStack.clear()
        pageStack.push(make(pages[name], { backend: backend }))
    }

    /// A recipe opens as a page beside the list (a column on a wide window, a page on a narrow one).
    function openRecipe(id) {
        while (pageStack.depth > 1) pageStack.pop()
        pageStack.push(make("RecipePage.qml", { backend: backend, recipeId: id }))
    }

    function configure(name) {
        pageStack.layers.push(Qt.resolvedUrl(name), { backend: backend })
    }

    /// Choose a day (and maybe a recipe) to log a meal for.
    function logMeal(day, recipeId) {
        logDialog.day = day
        logDialog.recipeId = recipeId
        logDialog.open()
    }

    readonly property Kirigami.Action appMenu: Kirigami.Action {
        icon.name: "application-menu"
        text: i18n("Menu")
        displayHint: Kirigami.DisplayHint.IconOnly
        Kirigami.Action {
            text: i18n("Configure Tags…"); icon.name: "tag"
            onTriggered: root.configure("TagsPage.qml")
        }
        Kirigami.Action {
            text: i18n("Configure Recipes…"); icon.name: "configure"
            shortcut: StandardKey.Preferences
            onTriggered: root.configure("PreferencesPage.qml")
        }
        Kirigami.Action {
            text: i18n("About Recipes"); icon.name: "help-about"
            onTriggered: pageStack.layers.push(root.make("shared/AboutPage.qml", {
                appName: "Recipes",
                appComment: "Keep your recipes, step by step, and what you cooked",
                appId: "io.github.vazark.Recipes"
            }))
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
        modal: Kirigami.Settings.isMobile || root.width < Kirigami.Units.gridUnit * 40
        drawerOpen: !modal
        onModalChanged: drawerOpen = !modal
        actions: [
            Kirigami.Action {
                text: i18n("Recipes")
                icon.name: "view-list-details"
                checked: root.section === "recipes"
                onTriggered: root.show("recipes")
            },
            Kirigami.Action {
                text: i18n("Log")
                icon.name: "view-calendar"
                checked: root.section === "calendar"
                onTriggered: root.show("calendar")
            },
            Kirigami.Action {
                text: i18n("Fridge")
                icon.name: "folder-cloud"
                checked: root.section === "fridge"
                onTriggered: root.show("fridge")
            },
            Kirigami.Action {
                text: i18n("Shopping List")
                icon.name: "view-task"
                checked: root.section === "shop"
                onTriggered: root.show("shop")
            }
        ]
    }

    LogMealDialog {
        id: logDialog
        backend: backend
    }

    Connections {
        target: backend
        function onToastChanged() {
            if (backend.toast !== "") root.showPassiveNotification(backend.toast)
        }
    }

    // Development: `--screenshot FILE` saves the window as a PNG and quits; `--page NAME`
    // (recipes, calendar, fridge, shop, tags, prefs, filter, about) and `--open-recipe ID`
    // choose what is shown.
    Timer {
        interval: 1200
        running: backend.arg("screenshot") !== ""
        onTriggered: {
            const page = backend.arg("page")
            if (root.pages[page]) root.show(page)
            else if (page === "tags") root.configure("TagsPage.qml")
            else if (page === "prefs") root.configure("PreferencesPage.qml")
            else if (page === "filter") root.configure("FilterPage.qml")
            else if (page === "about") pageStack.layers.push(root.make("shared/AboutPage.qml", { appName: "Recipes", appComment: "", appId: "io.github.vazark.Recipes" }))
            if (backend.arg("open-recipe") !== "") root.openRecipe(parseInt(backend.arg("open-recipe")))
            grabTimer.start()
        }
    }
    Timer {
        id: grabTimer
        interval: 900
        onTriggered: root.header.parent.grabToImage(r => { r.saveToFile(backend.arg("screenshot")); Qt.quit() })
    }

    Component.onCompleted: show("recipes")
}
