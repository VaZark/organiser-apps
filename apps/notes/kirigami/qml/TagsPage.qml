import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

/// Manage tags: a row each with its colour and how many notes use it; add, recolour, delete.
Kirigami.ScrollablePage {
    id: page
    required property var backend
    readonly property var tagList: JSON.parse(backend.tags)
    readonly property var presets: JSON.parse(backend.prefs).presets
    property string editing: ""

    title: i18n("Tags")

    actions: [
        Kirigami.Action {
            text: i18n("New Tag…")
            icon.name: "tag-new"
            shortcut: StandardKey.New
            onTriggered: newTagDialog.open()
        }
    ]

    Kirigami.PromptDialog {
        id: newTagDialog
        title: i18n("New Tag")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: { if (nameField.text.trim() !== "") page.backend.createTag(nameField.text, ""); nameField.text = "" }
        QQC2.TextField {
            id: nameField
            placeholderText: i18n("Tag name")
            onAccepted: newTagDialog.accept()
        }
    }

    Kirigami.Dialog {
        id: colorDialog
        title: i18n("Color of %1", page.editing)
        standardButtons: Kirigami.Dialog.Close
        ColumnLayout {
            spacing: Kirigami.Units.largeSpacing
            Flow {
                Layout.fillWidth: true
                Layout.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.smallSpacing
                Repeater {
                    model: page.presets
                    delegate: QQC2.AbstractButton {
                        id: sw
                        required property var modelData
                        implicitWidth: Kirigami.Units.gridUnit * 1.8
                        implicitHeight: implicitWidth
                        text: modelData.name
                        QQC2.ToolTip.text: text
                        QQC2.ToolTip.visible: hovered
                        onClicked: { page.backend.setTagColor(page.editing, modelData.hex); colorDialog.close() }
                        background: Rectangle { radius: width / 2; color: sw.modelData.hex; border.color: Kirigami.ColorUtils.linearInterpolation(Kirigami.Theme.backgroundColor, Kirigami.Theme.textColor, 0.25) }
                    }
                }
            }
            QQC2.TextField {
                Layout.fillWidth: true
                Layout.margins: Kirigami.Units.largeSpacing
                placeholderText: i18n("Custom color: #rrggbb")
                onAccepted: { page.backend.setTagColor(page.editing, text); colorDialog.close() }
            }
        }
    }

    ListView {
        model: page.tagList
        delegate: Kirigami.SwipeListItem {
            id: item
            required property var modelData
            contentItem: RowLayout {
                spacing: Kirigami.Units.largeSpacing
                Rectangle {
                    implicitWidth: Kirigami.Units.gridUnit
                    implicitHeight: implicitWidth
                    radius: width / 2
                    color: item.modelData.color ? item.modelData.color : Kirigami.Theme.disabledTextColor
                }
                QQC2.Label {
                    Layout.fillWidth: true
                    text: item.modelData.name + (item.modelData.parent ? "  (" + i18n("in %1", item.modelData.parent) + ")" : "")
                    elide: Text.ElideRight
                }
                QQC2.Label {
                    text: i18np("%1 note", "%1 notes", item.modelData.count)
                    color: Kirigami.Theme.disabledTextColor
                }
            }
            actions: [
                Kirigami.Action {
                    text: i18n("Change Color…")
                    icon.name: "color-management"
                    onTriggered: { page.editing = item.modelData.name; colorDialog.open() }
                },
                Kirigami.Action {
                    text: i18n("Delete Tag")
                    icon.name: "edit-delete"
                    onTriggered: page.backend.deleteTag(item.modelData.name, false)
                }
            ]
        }
        Kirigami.PlaceholderMessage {
            anchors.centerIn: parent
            width: parent.width - Kirigami.Units.gridUnit * 4
            visible: page.tagList.length === 0
            icon.name: "tag"
            text: i18n("No tags yet")
            explanation: i18n("Tags you add to notes appear here.")
        }
    }
}
