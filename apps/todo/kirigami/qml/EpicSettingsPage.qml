import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

/// Configure an epic: name, description, due date, colour, icon, and deleting it.
FormCard.FormCardPage {
    id: page
    required property var backend
    required property int epicId

    // Re-read when the data changes; the fields below are set once, so typing is not interrupted.
    readonly property var e: { backend.epics; return JSON.parse(backend.epicInfo(epicId)) }
    readonly property var presets: ["#e5484d", "#f76b15", "#f5d90a", "#46a758", "#12a594", "#3e63dd", "#8e4ec6", "#d6409f", "#8b8d98"]
    readonly property var icons: ["folder", "folder-favorites", "folder-documents", "folder-pictures", "folder-music", "folder-video", "folder-download", "user-home", "applications-development", "applications-office", "applications-games", "applications-internet", "emblem-favorite", "flag"]

    title: i18n("Configure %1", e.title ? e.title : i18n("Epic"))

    function close() { applicationWindow().pageStack.layers.pop() }

    FormCard.FormHeader { title: i18n("General") }
    FormCard.FormCard {
        FormCard.FormTextFieldDelegate {
            label: i18n("Name")
            Component.onCompleted: text = page.e.title ? page.e.title : ""
            onEditingFinished: if (text.trim() !== "") page.backend.setEpicTitle(page.epicId, text)
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormTextAreaDelegate {
            label: i18n("Description")
            Component.onCompleted: text = page.e.notes ? page.e.notes : ""
            onEditingFinished: page.backend.setEpicNotes(page.epicId, text)
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormDateTimeDelegate {
            text: i18n("Due date")
            dateTimeDisplay: FormCard.FormDateTimeDelegate.DateTimeDisplay.Date
            resettable: true
            // An unset date is an invalid Date. Only a real change is written back (the binding
            // below also fires onValueChanged when the page loads or the epic changes).
            value: page.e.due ? new Date(page.e.due + "T00:00:00") : new Date(NaN)
            onValueChanged: {
                const iso = isNaN(value.valueOf()) ? "" : Qt.formatDate(value, "yyyy-MM-dd")
                if (iso !== (page.e.due ? page.e.due : "")) page.backend.setEpicDue(page.epicId, iso)
            }
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormTextDelegate {
            text: i18n("Progress")
            description: i18n("%1 open of %2 tasks", page.e.open === undefined ? 0 : page.e.open, page.e.total === undefined ? 0 : page.e.total)
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
                            required property string modelData
                            implicitWidth: Kirigami.Units.gridUnit * 1.6
                            implicitHeight: implicitWidth
                            onClicked: page.backend.setEpicColor(page.epicId, modelData)
                            background: Rectangle {
                                radius: width / 2
                                color: swatch.modelData
                                border.width: page.e.color === swatch.modelData ? 3 : 1
                                border.color: page.e.color === swatch.modelData ? Kirigami.Theme.highlightColor : Kirigami.ColorUtils.linearInterpolation(Kirigami.Theme.backgroundColor, Kirigami.Theme.textColor, 0.25)
                            }
                        }
                    }
                    QQC2.ToolButton {
                        icon.name: "edit-clear"
                        text: i18n("No Color")
                        display: QQC2.AbstractButton.IconOnly
                        QQC2.ToolTip.text: text
                        QQC2.ToolTip.visible: hovered
                        enabled: !!page.e.color
                        onClicked: page.backend.setEpicColor(page.epicId, "")
                    }
                }
            }
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormTextFieldDelegate {
            label: i18n("Custom color")
            placeholderText: "#rrggbb"
            Component.onCompleted: text = page.e.color ? page.e.color : ""
            onAccepted: page.backend.setEpicColor(page.epicId, text)
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
                        model: page.icons
                        delegate: QQC2.ToolButton {
                            required property string modelData
                            icon.name: modelData
                            display: QQC2.AbstractButton.IconOnly
                            checkable: true
                            checked: (page.e.icon ? page.e.icon : "folder") === modelData
                            QQC2.ToolTip.text: modelData
                            QQC2.ToolTip.visible: hovered
                            onClicked: page.backend.setEpicIcon(page.epicId, modelData)
                        }
                    }
                }
            }
        }
    }

    FormCard.FormHeader { title: i18n("Epic") }
    FormCard.FormCard {
        FormCard.FormButtonDelegate {
            text: i18n("Delete Epic…")
            icon.name: "edit-delete"
            onClicked: deleteDialog.open()
        }
    }

    Kirigami.PromptDialog {
        id: deleteDialog
        title: i18n("Delete this epic?")
        subtitle: i18n("Keep its tasks (they leave the epic) or delete them too.")
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: i18n("Keep Tasks"); icon.name: "edit-delete"
                onTriggered: { deleteDialog.close(); page.backend.deleteEpic(page.epicId, false); page.close() }
            },
            Kirigami.Action {
                text: i18n("Delete Tasks Too"); icon.name: "edit-delete"
                onTriggered: { deleteDialog.close(); page.backend.deleteEpic(page.epicId, true); page.close() }
            }
        ]
    }
}
