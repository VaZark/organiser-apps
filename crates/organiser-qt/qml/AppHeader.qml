import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

/// The window-wide header: sidebar toggle (when the sidebar is a drawer), back, and the page's
/// actions. No title: the sidebar already says where you are. It spans the full window, with the
/// sidebar and the content under it. On a narrow ("mobile") window the actions show icons only.
QQC2.ToolBar {
    id: bar
    required property var window
    readonly property var pageStack: window.pageStack
    readonly property bool inLayer: pageStack.layers.depth > 1
    // What the person is looking at: a pushed layer (settings) or the page stack's current page.
    readonly property var page: inLayer ? pageStack.layers.currentItem : pageStack.currentItem
    // The sidebar is a drawer instead of a docked panel: a narrow window or a touch device.
    readonly property bool mobile: window.globalDrawer ? window.globalDrawer.modal : false
    /// The page's actions show icons only (their names are the tooltips), as on a narrow window.
    property bool iconOnly: false
    /// Menus of the page's actions open as a bottom sheet instead of a dropdown, and the actions
    /// the page hides in the overflow are listed under a "More" icon in a sheet too.
    property bool sheets: false
    readonly property bool canGoBack: inLayer || pageStack.depth > 1

    // Back: its button is at the left edge of the header (below).
    Kirigami.Action {
        id: backAction
        visible: bar.canGoBack
        icon.name: "go-previous"
        text: i18n("Back")
        displayHint: Kirigami.DisplayHint.IconOnly | Kirigami.DisplayHint.KeepVisible
        onTriggered: bar.inLayer ? bar.pageStack.layers.pop() : bar.pageStack.pop()
    }

    // ---- sheets ------------------------------------------------------------------------------
    readonly property var pageActions: bar.page && bar.page.actions ? Array.from(bar.page.actions) : []
    readonly property var hiddenActions: bar.sheets
        ? bar.pageActions.filter(a => a && (a.displayHint & Kirigami.DisplayHint.AlwaysHide)) : []
    property var proxied: []
    function collect() {
        const list = []
        for (let i = 0; i < proxies.count; ++i) {
            const o = proxies.objectAt(i)
            if (o) list.push(o)
        }
        proxied = list
    }

    ActionDrawer { id: sheet }

    // One action per page action, drawn in its place: a menu opens the sheet, anything else is
    // forwarded to the action itself.
    Instantiator {
        id: proxies
        model: bar.sheets ? bar.pageActions.filter(a => a && !(a.displayHint & Kirigami.DisplayHint.AlwaysHide)) : []
        delegate: Kirigami.Action {
            id: proxy
            required property var modelData
            text: modelData.text
            icon.name: modelData.icon ? modelData.icon.name : ""
            displayHint: modelData.displayHint
            visible: modelData.visible
            enabled: modelData.enabled
            separator: modelData.separator
            checkable: modelData.checkable
            checked: modelData.checked
            onTriggered: {
                if (modelData.children !== undefined && modelData.children.length > 0) sheet.showMenu(modelData)
                else modelData.trigger()
                // Toggling the stand-in must not cut it off from the action it shows.
                if (checkable) checked = Qt.binding(() => proxy.modelData.checked)
            }
        }
        onObjectAdded: Qt.callLater(bar.collect)
        onObjectRemoved: Qt.callLater(bar.collect)
    }
    // Narrow, where the list is a bottom sheet: the bar keeps only Back and one menu icon, and
    // every other action is in the sheet (a choice such as Sort By is a row that opens one level deeper).
    readonly property bool collapsed: bar.sheets
        && (Kirigami.Settings.isMobile || bar.window.width < Kirigami.Units.gridUnit * 30)
    function everything() {
        return bar.pageActions.filter(a => a && a.visible !== false)
    }
    // A page with a bottom bar (`pageBar: true`) keeps its own actions there when narrow, so the
    // top has only the app's global menu: one icon, opening the window's `appMenu` as a sheet.
    readonly property bool pageBar: bar.page && bar.page.pageBar === true
    Kirigami.Action {
        id: globalAction
        text: bar.window.appMenu ? bar.window.appMenu.text : ""
        icon.name: bar.window.appMenu && bar.window.appMenu.icon ? bar.window.appMenu.icon.name : "application-menu"
        displayHint: Kirigami.DisplayHint.IconOnly
        visible: !!bar.window.appMenu
        onTriggered: sheet.showMenu(bar.window.appMenu)
    }
    Kirigami.Action {
        id: allAction
        text: i18n("Menu")
        icon.name: "application-menu"
        displayHint: Kirigami.DisplayHint.IconOnly
        visible: bar.pageActions.length > 0
        onTriggered: sheet.showActions("", bar.everything())
    }
    Kirigami.Action {
        id: moreAction
        text: i18n("More")
        icon.name: "overflow-menu"
        displayHint: Kirigami.DisplayHint.IconOnly
        visible: bar.hiddenActions.length > 0
        onTriggered: sheet.showActions("", bar.hiddenActions)
    }

    contentItem: RowLayout {
        spacing: Kirigami.Units.smallSpacing
        // Back, at the left edge. Where there is a page to go back from the sidebar is not part of
        // the context (it lists what the list shows), so Back takes the sidebar button's place.
        QQC2.ToolButton {
            visible: bar.canGoBack
            icon.name: backAction.icon.name
            text: backAction.text
            display: QQC2.AbstractButton.IconOnly
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            onClicked: backAction.trigger()
        }
        QQC2.ToolButton {
            visible: bar.mobile && !bar.canGoBack
            icon.name: "sidebar-expand-left"
            text: i18n("Sidebar")
            display: QQC2.AbstractButton.IconOnly
            onClicked: bar.window.globalDrawer.drawerOpen = !bar.window.globalDrawer.drawerOpen
        }
        Kirigami.ActionToolBar {
            Layout.fillWidth: true
            alignment: Qt.AlignRight
            display: bar.mobile || bar.iconOnly ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
            actions: bar.collapsed && bar.pageBar ? [globalAction]
                : bar.collapsed ? [allAction]
                : bar.sheets ? bar.proxied.concat([moreAction])
                : bar.pageActions
        }
    }
}
