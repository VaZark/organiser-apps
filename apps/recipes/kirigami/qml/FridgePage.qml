import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.recipes

/// What you have, with an optional expiry date. Past its date an item no longer counts as in
/// stock when the shopping list is worked out.
Kirigami.ScrollablePage {
    id: page
    required property var backend
    title: i18n("Fridge")
    readonly property var stock: JSON.parse(backend.stock)

    // The item being edited (-1: a new one).
    property int editId: -1
    function edit(item) {
        editId = item ? item.id : -1
        nameField.text = item ? item.name : ""
        qtyField.text = item ? item.qty : ""
        unitField.text = item ? item.unit : ""
        expiresField.text = item ? item.expires : ""
        dialog.open()
    }

    Kirigami.Dialog {
        id: dialog
        title: page.editId < 0 ? i18n("Add Item") : i18n("Edit Item")
        standardButtons: Kirigami.Dialog.NoButton
        customFooterActions: [
            Kirigami.Action {
                text: page.editId < 0 ? i18n("Add") : i18n("Save")
                icon.name: "dialog-ok"
                onTriggered: {
                    const ok = page.editId < 0
                        ? page.backend.addStock(nameField.text, qtyField.text, unitField.text, expiresField.text)
                        : page.backend.editStock(page.editId, nameField.text, qtyField.text, unitField.text, expiresField.text)
                    if (ok) dialog.close()
                }
            },
            Kirigami.Action { text: i18n("Cancel"); icon.name: "dialog-cancel"; onTriggered: dialog.close() }
        ]
        Kirigami.FormLayout {
            QQC2.TextField { id: nameField; Kirigami.FormData.label: i18n("Name:"); Layout.fillWidth: true }
            QQC2.TextField { id: qtyField; Kirigami.FormData.label: i18n("Quantity:") }
            QQC2.TextField { id: unitField; Kirigami.FormData.label: i18n("Unit:") }
            QQC2.TextField { id: expiresField; Kirigami.FormData.label: i18n("Expires:"); placeholderText: "2026-11-30" }
        }
    }

    actions: Kirigami.Action {
        text: i18n("Add Item…")
        icon.name: "list-add"
        shortcut: StandardKey.New
        onTriggered: page.edit(null)
    }

    ListView {
        id: list
        model: page.stock
        delegate: Kirigami.SwipeListItem {
            id: row
            required property var modelData
            required property int index
            onClicked: page.edit(modelData)
            contentItem: RowLayout {
                spacing: Kirigami.Units.largeSpacing
                QQC2.Label {
                    Layout.fillWidth: true
                    text: row.modelData.name
                    elide: Text.ElideRight
                    color: row.modelData.expired ? Kirigami.Theme.disabledTextColor : Kirigami.Theme.textColor
                    font.strikeout: row.modelData.expired
                }
                QQC2.Label { text: row.modelData.qty + (row.modelData.unit !== "" ? " " + row.modelData.unit : ""); color: Kirigami.Theme.disabledTextColor }
                QQC2.Label {
                    visible: row.modelData.expires !== ""
                    text: row.modelData.expired ? i18n("Expired %1", row.modelData.expires) : i18n("Expires %1", row.modelData.expires)
                    color: row.modelData.expired ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.disabledTextColor
                }
            }
            actions: [
                Kirigami.Action { text: i18n("Move Up"); icon.name: "go-up"; enabled: row.index > 0; onTriggered: page.backend.moveStock(row.modelData.id, -1) },
                Kirigami.Action { text: i18n("Move Down"); icon.name: "go-down"; enabled: row.index < page.stock.length - 1; onTriggered: page.backend.moveStock(row.modelData.id, 1) },
                Kirigami.Action { text: i18n("Delete"); icon.name: "edit-delete"; onTriggered: page.backend.deleteStock(row.modelData.id) }
            ]
        }
        EmptyState {
            visible: list.count === 0
            icon.name: "folder-cloud"
            text: i18n("The fridge is empty")
            explanation: i18n("Add what you have; the shopping list only asks for what is missing.")
            helpfulAction: Kirigami.Action { text: i18n("Add Item…"); icon.name: "list-add"; onTriggered: page.edit(null) }
        }
    }
}
