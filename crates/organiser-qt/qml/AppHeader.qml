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
    readonly property bool canGoBack: inLayer || pageStack.depth > 1

    // Back sits at the left edge of the block of action icons on the right.
    Kirigami.Action {
        id: backAction
        visible: bar.canGoBack
        icon.name: "go-previous"
        text: i18n("Back")
        displayHint: Kirigami.DisplayHint.IconOnly | Kirigami.DisplayHint.KeepVisible
        onTriggered: bar.inLayer ? bar.pageStack.layers.pop() : bar.pageStack.pop()
    }

    contentItem: RowLayout {
        spacing: Kirigami.Units.smallSpacing
        QQC2.ToolButton {
            visible: bar.mobile
            icon.name: "sidebar-expand-left"
            text: i18n("Sidebar")
            display: QQC2.AbstractButton.IconOnly
            onClicked: bar.window.globalDrawer.drawerOpen = !bar.window.globalDrawer.drawerOpen
        }
        Kirigami.ActionToolBar {
            Layout.fillWidth: true
            alignment: Qt.AlignRight
            display: bar.mobile ? QQC2.AbstractButton.IconOnly : QQC2.AbstractButton.TextBesideIcon
            actions: [backAction].concat(bar.page && bar.page.actions ? Array.from(bar.page.actions) : [])
        }
    }
}
