import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.todo

/// The person's statuses: order is the board's column order.
Kirigami.ScrollablePage {
    id: page
    required property var backend
    title: i18n("Statuses")
    actions: Kirigami.Action { text: i18n("New status"); icon.name: "list-add"; onTriggered: { editor.statusId = -1; nameField.text = ""; colorField.text = ""; closedBox.checked = false; editor.open() } }

    Kirigami.Dialog {
        id: editor
        property int statusId: -1
        title: statusId < 0 ? i18n("New status") : i18n("Edit status")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        ColumnLayout {
            QQC2.TextField { id: nameField; placeholderText: i18n("Name"); Layout.fillWidth: true }
            QQC2.TextField { id: colorField; placeholderText: i18n("Colour (#rrggbb)"); Layout.fillWidth: true }
            QQC2.CheckBox { id: closedBox; text: i18n("Means finished") }
        }
        onAccepted: statusId < 0
            ? page.backend.createStatus(nameField.text, colorField.text, closedBox.checked)
            : page.backend.editStatus(statusId, nameField.text, colorField.text, closedBox.checked)
    }

    ListView {
        model: JSON.parse(page.backend.statuses)
        delegate: QQC2.ItemDelegate {
            id: d
            required property var modelData
            width: ListView.view.width
            onClicked: { editor.statusId = modelData.id; nameField.text = modelData.name; colorField.text = modelData.color ? modelData.color : ""; closedBox.checked = modelData.closed; editor.open() }
            contentItem: RowLayout {
                StatusDot { tint: d.modelData.color ? d.modelData.color : Kirigami.Theme.disabledTextColor }
                QQC2.Label { Layout.fillWidth: true; text: d.modelData.name + (d.modelData.closed ? i18n(" (finished)") : "") }
                QQC2.ToolButton { icon.name: "go-up"; onClicked: page.backend.moveStatus(d.modelData.id, -1) }
                QQC2.ToolButton { icon.name: "go-down"; onClicked: page.backend.moveStatus(d.modelData.id, 1) }
                QQC2.ToolButton { icon.name: "edit-delete"; onClicked: page.backend.deleteStatus(d.modelData.id) }
            }
        }
    }
}
