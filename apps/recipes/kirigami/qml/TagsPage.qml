import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import org.kde.kirigami as Kirigami
import org.organiser.recipes

/// The tag vocabulary: a tag may sit under another, and has a colour of its own.
Kirigami.ScrollablePage {
    id: page
    required property var backend
    title: i18n("Configure Tags")
    readonly property var tags: JSON.parse(backend.tags)
    property string target

    actions: Kirigami.Action {
        text: i18n("New Tag…")
        icon.name: "list-add"
        shortcut: StandardKey.New
        onTriggered: newTag.open()
    }

    Kirigami.Dialog {
        id: newTag
        title: i18n("New Tag")
        standardButtons: Kirigami.Dialog.NoButton
        customFooterActions: [
            Kirigami.Action {
                text: i18n("Create")
                icon.name: "dialog-ok"
                enabled: newTagName.text.trim() !== ""
                onTriggered: {
                    page.backend.createTag(newTagName.text, tagParent.currentIndex > 0 ? tagParent.currentText : "")
                    newTagName.text = ""
                    tagParent.currentIndex = 0
                    newTag.close()
                }
            },
            Kirigami.Action { text: i18n("Cancel"); icon.name: "dialog-cancel"; onTriggered: newTag.close() }
        ]
        Kirigami.FormLayout {
            QQC2.TextField { id: newTagName; Kirigami.FormData.label: i18n("Name:") }
            QQC2.ComboBox {
                id: tagParent
                Kirigami.FormData.label: i18n("Under:")
                model: [i18n("No parent")].concat(page.tags.map(t => t.name))
            }
        }
    }

    Kirigami.Dialog {
        id: moveTag
        title: i18n("Move %1", page.target)
        standardButtons: Kirigami.Dialog.NoButton
        customFooterActions: [
            Kirigami.Action {
                text: i18n("Move")
                icon.name: "dialog-ok"
                onTriggered: { page.backend.setTagParent(page.target, moveParent.currentIndex > 0 ? moveParent.currentText : ""); moveTag.close() }
            },
            Kirigami.Action { text: i18n("Cancel"); icon.name: "dialog-cancel"; onTriggered: moveTag.close() }
        ]
        QQC2.ComboBox {
            id: moveParent
            implicitWidth: Kirigami.Units.gridUnit * 14
            model: [i18n("No parent")].concat(page.tags.map(t => t.name).filter(n => n !== page.target))
        }
    }

    ColorDialog {
        id: colorDialog
        title: i18n("Tag Color")
        onAccepted: page.backend.setTagColor(page.target, selectedColor.toString())
    }

    ListView {
        id: list
        model: page.tags
        delegate: Kirigami.SwipeListItem {
            id: row
            required property var modelData
            leftPadding: Kirigami.Units.largeSpacing + (modelData.parent ? Kirigami.Units.gridUnit * 1.5 : 0)
            contentItem: RowLayout {
                spacing: Kirigami.Units.largeSpacing
                StatusDot { tint: row.modelData.color ? row.modelData.color : Kirigami.Theme.disabledTextColor }
                QQC2.Label { Layout.fillWidth: true; text: row.modelData.name }
            }
            actions: [
                Kirigami.Action {
                    text: i18n("Change Color…")
                    icon.name: "color-management"
                    onTriggered: { page.target = row.modelData.name; if (row.modelData.color) colorDialog.selectedColor = row.modelData.color; colorDialog.open() }
                },
                Kirigami.Action {
                    text: i18n("Move Under…")
                    icon.name: "go-parent-folder"
                    onTriggered: { page.target = row.modelData.name; moveParent.currentIndex = 0; moveTag.open() }
                },
                Kirigami.Action {
                    text: i18n("Delete")
                    icon.name: "edit-delete"
                    onTriggered: page.backend.deleteTag(row.modelData.name, false)
                }
            ]
        }
        EmptyState {
            visible: list.count === 0
            icon.name: "tag"
            text: i18n("No tags yet")
            explanation: i18n("Add tags to a recipe from its page.")
        }
    }
}
