import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

/// Configure a folder: name, description, ratings, colour, icon, where it is, and deleting it.
FormCard.FormCardPage {
    id: page
    required property var backend
    required property int folderId

    // Re-read when the data changes; the fields below are set once, so typing is not interrupted.
    readonly property var f: { backend.page; return JSON.parse(backend.folder(folderId)) }
    readonly property var presets: JSON.parse(backend.prefs).presets

    title: i18n("Configure %1", f.title ? f.title : i18n("Folder"))

    function close() { applicationWindow().pageStack.layers.pop() }

    FormCard.FormHeader { title: i18n("General") }
    FormCard.FormCard {
        FormCard.FormTextFieldDelegate {
            label: i18n("Name")
            Component.onCompleted: text = page.f.title
            onEditingFinished: page.backend.setFolderTitle(page.folderId, text)
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormTextAreaDelegate {
            label: i18n("Description")
            Component.onCompleted: text = page.f.notes
            onEditingFinished: page.backend.setFolderNotes(page.folderId, text)
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormSwitchDelegate {
            text: i18n("Rate notes")
            description: i18n("Notes in this folder show a star rating, used in searching and sorting. Switching it off keeps the ratings.")
            checked: page.f.rated === true
            onToggled: page.backend.setFolderRated(page.folderId, checked)
        }
        FormCard.FormDelegateSeparator { visible: !page.f.isDefault }
        FormCard.FormComboBoxDelegate {
            visible: !page.f.isDefault
            text: i18n("Inside")
            description: i18n("Where this folder is kept")
            textRole: "title"
            valueRole: "id"
            model: page.f.parentChoices ? page.f.parentChoices : []
            Component.onCompleted: currentIndex = Math.max(0, model.findIndex(c => c.id === (page.f.parent === null ? -1 : page.f.parent)))
            onActivated: page.backend.moveFolderInto(page.folderId, currentValue)
        }
    }

    FormCard.FormHeader { title: i18n("Appearance") }
    FormCard.FormCard {
        FormCard.AbstractFormDelegate {
            background: null
            text: i18n("Color")
            contentItem: ColumnLayout {
                spacing: Kirigami.Units.smallSpacing
                QQC2.Label { text: i18n("Color"); Layout.fillWidth: true }
                Flow {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing
                    Repeater {
                        model: page.presets
                        delegate: QQC2.AbstractButton {
                            id: swatch
                            required property var modelData
                            implicitWidth: Kirigami.Units.gridUnit * 1.6
                            implicitHeight: implicitWidth
                            text: modelData.name
                            QQC2.ToolTip.text: text
                            QQC2.ToolTip.visible: hovered
                            onClicked: page.backend.setFolderColor(page.folderId, modelData.hex)
                            background: Rectangle {
                                radius: width / 2
                                color: swatch.modelData.hex
                                border.width: page.f.ownColor === swatch.modelData.hex ? 3 : 1
                                border.color: page.f.ownColor === swatch.modelData.hex ? Kirigami.Theme.highlightColor : Kirigami.ColorUtils.linearInterpolation(Kirigami.Theme.backgroundColor, Kirigami.Theme.textColor, 0.25)
                            }
                        }
                    }
                    QQC2.ToolButton {
                        icon.name: "edit-clear"
                        text: i18n("Default Color")
                        display: QQC2.AbstractButton.IconOnly
                        QQC2.ToolTip.text: text
                        QQC2.ToolTip.visible: hovered
                        enabled: page.f.ownColor !== null && page.f.ownColor !== undefined
                        onClicked: page.backend.setFolderColor(page.folderId, "")
                    }
                }
            }
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormTextFieldDelegate {
            label: i18n("Custom color")
            placeholderText: "#rrggbb"
            Component.onCompleted: text = page.f.ownColor ? page.f.ownColor : ""
            onAccepted: page.backend.setFolderColor(page.folderId, text)
        }
        FormCard.FormDelegateSeparator {}
        FormCard.AbstractFormDelegate {
            background: null
            text: i18n("Icon")
            contentItem: ColumnLayout {
                spacing: Kirigami.Units.smallSpacing
                QQC2.Label { text: i18n("Icon"); Layout.fillWidth: true }
                Flow {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing
                    Repeater {
                        model: page.f.icons ? page.f.icons : []
                        delegate: QQC2.ToolButton {
                            required property var modelData
                            icon.name: modelData.icon
                            text: modelData.label
                            display: QQC2.AbstractButton.IconOnly
                            checkable: true
                            checked: page.f.iconName === modelData.name
                            QQC2.ToolTip.text: text
                            QQC2.ToolTip.visible: hovered
                            onClicked: page.backend.setFolderIcon(page.folderId, modelData.name)
                        }
                    }
                }
            }
        }
    }

    FormCard.FormHeader { title: i18n("Folder"); visible: !page.f.isDefault }
    FormCard.FormCard {
        visible: !page.f.isDefault
        FormCard.FormButtonDelegate {
            text: i18n("Delete Folder…")
            icon.name: "edit-delete"
            onClicked: deleteDialog.open()
        }
    }

    Kirigami.PromptDialog {
        id: deleteDialog
        title: i18n("Delete Folder")
        subtitle: page.f.count > 0
            ? i18np("This folder has %1 note. Keep it (it moves to the folder this one is in), or delete it with the folder?",
                    "This folder has %1 notes. Keep them (they move to the folder this one is in), or delete them with the folder?", page.f.count)
            : i18n("Delete this folder? Folders inside it stay.")
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: i18n("Keep Notes")
                icon.name: "folder"
                visible: page.f.count > 0
                onTriggered: { deleteDialog.close(); const id = page.folderId; page.close(); page.backend.deleteFolder(id, false) }
            },
            Kirigami.Action {
                text: i18n("Delete Notes Too")
                icon.name: "edit-delete"
                visible: page.f.count > 0
                onTriggered: { deleteDialog.close(); const id = page.folderId; page.close(); page.backend.deleteFolder(id, true) }
            },
            Kirigami.Action {
                text: i18n("Delete")
                icon.name: "edit-delete"
                visible: !(page.f.count > 0)
                onTriggered: { deleteDialog.close(); const id = page.folderId; page.close(); page.backend.deleteFolder(id, false) }
            }
        ]
    }
}
