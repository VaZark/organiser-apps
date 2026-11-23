import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

/// A list of actions that is a menu in a wide window and a bottom sheet in a narrow one (or on a
/// touch device). Either way it stays inside the window it is in: the menu is a popup there, and
/// the sheet is as wide as the window and at most four fifths as high.
///
/// What it lists are action-like objects: a `Kirigami.Action` or `QQC2.Action`, or a plain object
/// `{text, icon: {name, color}, checkable, checked, enabled, trigger: function}`. An action with
/// sub-actions (`children`) is a submenu: a submenu of the menu, or a row with a chevron in the sheet
/// that opens the choices one level deeper (with a Back arrow), which is where a choice such as
/// "Sort By" or "View" goes. There are no dividers: `separator` actions are ignored.
///
///     ActionDrawer { id: sheet }
///     onClicked: sheet.showActions(i18n("Page"), [first, sortActionWithChildren, last], this)
///     onClicked: sheet.showMenu(someActionWithChildren)
Item {
    id: root
    // Nothing to draw here: the menu and the sheet are popups.
    width: 0
    height: 0

    /// Below this width of the window the list is a bottom sheet.
    property real narrowWidth: Kirigami.Units.gridUnit * 30

    /// Whether the window is narrow, looked at when the list opens (not bound: the window's size is
    /// not asked for while this is being made).
    property bool narrow: false
    function checkNarrow() {
        const w = root.Window.width
        narrow = Kirigami.Settings.isMobile || (w > 0 && w < narrowWidth)
    }

    /// The levels of the sheet, the deepest last: `{ title, entries }`. A choice opens a level.
    property var levels: []
    readonly property var current: levels.length > 0 ? levels[levels.length - 1] : ({ title: "", entries: [] })

    /// The entries of a list of actions: what is hidden when the list opens is left out, and an
    /// action with sub-actions keeps them as `children`, entries of their own.
    function flatten(list) {
        const out = []
        for (const a of list) {
            if (!a || a.separator === true || a.visible === false) continue
            if (a.children !== undefined && a.children.length > 0) {
                const kids = flatten(Array.from(a.children))
                if (kids.length > 0) out.push({ kind: "submenu", action: a, children: kids })
            } else {
                out.push({ kind: "action", action: a })
            }
        }
        return out
    }

    /// Show `list`. `anchor` (an item, optional) is where the menu drops down from; without it
    /// the menu opens at the pointer. A sheet ignores it.
    function showActions(title, list, anchor) {
        checkNarrow()
        const entries = flatten(list)
        if (root.narrow) {
            root.levels = [{ title: title, entries: entries }]
            // The window's overlay; a plain `Window` has none to name, its content item stands in.
            sheet.parent = QQC2.Overlay.overlay ? QQC2.Overlay.overlay : root.Window.contentItem
            sheet.open()
        } else {
            clearMenu()
            fillMenu(menu, entries)
            if (anchor) menu.popup(anchor, 0, anchor.height)
            else menu.popup()
        }
    }

    /// The sub-actions of a `Kirigami.Action` (a menu), under its own name.
    function showMenu(action, anchor) {
        showActions(action.text, Array.from(action.children), anchor)
    }

    // ---- the menu (wide) ---------------------------------------------------------------------
    function clearMenu() {
        while (menu.count > 0) {
            const old = menu.takeItem(0)
            if (old) {
                if (old.subMenu) old.subMenu.destroy()
                old.destroy()
            }
        }
    }
    function fillMenu(target, entries) {
        for (const e of entries) {
            if (e.kind === "submenu") {
                const sub = subMenu.createObject(null, { title: e.action.text })
                fillMenu(sub, e.children)
                target.addMenu(sub)
            } else {
                const item = actionItem.createObject(null, { act: e.action })
                if (item) target.addItem(item)
            }
        }
    }

    QQC2.Menu {
        id: menu
    }
    Component { id: subMenu; QQC2.Menu {} }
    Component {
        id: actionItem
        QQC2.MenuItem {
            property var act
            text: act.text
            icon.name: act.icon && act.icon.name ? act.icon.name : ""
            icon.color: act.icon && act.icon.color !== undefined ? act.icon.color : Kirigami.Theme.textColor
            enabled: act.enabled !== false
            checkable: act.checkable === true
            checked: act.checkable === true && act.checked === true
            onTriggered: act.trigger()
        }
    }

    // ---- the sheet (narrow) ------------------------------------------------------------------
    QQC2.Drawer {
        id: sheet
        // Set when it opens (see `showActions`): the window's overlay, so the sheet is bounded by the
        // window it is in. Touching the overlay of a window that is not shown yet stalls the start.
        edge: Qt.BottomEdge
        modal: true
        width: parent ? parent.width : implicitWidth
        height: Math.min((parent ? parent.height : 0) * 0.8,
            (title.visible ? title.implicitHeight + title.Layout.bottomMargin : 0) + column.implicitHeight
            + topPadding + bottomPadding)
        topPadding: Kirigami.Units.smallSpacing
        bottomPadding: Kirigami.Units.smallSpacing
        leftPadding: 0
        rightPadding: 0
        // The next time it opens it starts at the top level.
        onClosed: root.levels = root.levels.slice(0, 1)

        contentItem: ColumnLayout {
            spacing: 0
            // The title; in a deeper level, with Back to the level above.
            RowLayout {
                id: title
                visible: root.current.title !== "" || root.levels.length > 1
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.smallSpacing
                Layout.rightMargin: Kirigami.Units.largeSpacing
                Layout.bottomMargin: Kirigami.Units.smallSpacing
                spacing: Kirigami.Units.smallSpacing
                QQC2.ToolButton {
                    visible: root.levels.length > 1
                    icon.name: "go-previous"
                    text: i18n("Back")
                    display: QQC2.AbstractButton.IconOnly
                    onClicked: root.levels = root.levels.slice(0, -1)
                }
                Kirigami.Heading {
                    level: 4
                    text: root.current.title
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                    Layout.leftMargin: root.levels.length > 1 ? 0 : Kirigami.Units.smallSpacing
                }
            }
            // Every entry is made (there are few), so the sheet knows its exact height.
            Flickable {
                id: view
                Layout.fillWidth: true
                Layout.fillHeight: true
                // Its content's height, so the layout (and the sheet) is as high as the list.
                Layout.preferredHeight: column.implicitHeight
                clip: true
                contentWidth: width
                contentHeight: column.implicitHeight
                boundsBehavior: Flickable.StopAtBounds
                ColumnLayout {
                    id: column
                    width: view.width
                    spacing: 0
                    Repeater {
                        model: root.current.entries
                        delegate: QQC2.ItemDelegate {
                            id: row
                            required property var modelData
                            readonly property var act: modelData.action
                            readonly property bool submenu: modelData.kind === "submenu"
                            Layout.fillWidth: true
                            enabled: act.enabled !== false
                            text: act.text
                            icon.name: act.icon && act.icon.name ? act.icon.name : ""
                            contentItem: RowLayout {
                                spacing: Kirigami.Units.largeSpacing
                                Kirigami.Icon {
                                    visible: row.icon.name !== ""
                                    source: row.icon.name
                                    isMask: row.act.icon && row.act.icon.color !== undefined
                                    color: row.act.icon && row.act.icon.color !== undefined ? row.act.icon.color : "transparent"
                                    Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
                                    Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
                                }
                                QQC2.Label {
                                    Layout.fillWidth: true
                                    text: row.text
                                    elide: Text.ElideRight
                                }
                                // A choice that is on.
                                Kirigami.Icon {
                                    visible: !row.submenu && row.act.checkable === true && row.act.checked === true
                                    source: "checkmark"
                                    Layout.preferredWidth: Kirigami.Units.iconSizes.small
                                    Layout.preferredHeight: Kirigami.Units.iconSizes.small
                                }
                                // A submenu: its choices are one level deeper.
                                Kirigami.Icon {
                                    visible: row.submenu
                                    source: "go-next"
                                    Layout.preferredWidth: Kirigami.Units.iconSizes.small
                                    Layout.preferredHeight: Kirigami.Units.iconSizes.small
                                }
                            }
                            onClicked: {
                                if (row.submenu) {
                                    root.levels = root.levels.concat([{ title: row.act.text, entries: modelData.children }])
                                } else {
                                    sheet.close()
                                    row.act.trigger()
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
