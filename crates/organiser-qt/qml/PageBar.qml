import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

/// The foot of a list page on a narrow window: up to three icons in one row, and a ⋮ menu for the
/// rest of the page's own actions (its filters and options). The app's global menu stays at the
/// top. The list is a bottom sheet in a narrow window, which is where this bar is shown.
///
///     footer: PageBar { icons: [addAction, filterAction]; menu: [sortAction, showAction] }
QQC2.ToolBar {
    id: bar
    /// Actions drawn as icons, at most three. One with sub-actions opens them as a sheet.
    property var icons: []
    /// Everything else, in the ⋮ menu (an action with sub-actions is a row that opens its choices one level deeper).
    property var menu: []
    property string heading: ""

    position: QQC2.ToolBar.Footer

    ActionDrawer { id: sheet }

    contentItem: RowLayout {
        spacing: Kirigami.Units.smallSpacing
        Repeater {
            model: bar.icons.slice(0, 3)
            delegate: QQC2.ToolButton {
                id: button
                required property var modelData
                visible: modelData.visible !== false
                enabled: modelData.enabled !== false
                icon.name: modelData.icon ? modelData.icon.name : ""
                text: modelData.text
                display: QQC2.AbstractButton.IconOnly
                checkable: modelData.checkable === true
                checked: modelData.checkable === true && modelData.checked === true
                QQC2.ToolTip.text: text
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                onClicked: {
                    if (modelData.children !== undefined && modelData.children.length > 0) sheet.showMenu(modelData, button)
                    else modelData.trigger()
                }
            }
        }
        Item { Layout.fillWidth: true }
        QQC2.ToolButton {
            id: more
            visible: bar.menu.length > 0
            text: i18n("More")
            icon.name: "overflow-menu"
            display: QQC2.AbstractButton.IconOnly
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            onClicked: sheet.showActions(bar.heading, bar.menu, more)
        }
    }
}
