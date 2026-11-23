import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import org.kde.kirigami as Kirigami
import org.organiser.recipes

/// A shopping list prepared on request: the recipes you pick (portions each), set against what the
/// fridge has and has not let expire.
Kirigami.ScrollablePage {
    id: page
    required property var backend
    title: i18n("Shopping List")
    readonly property var list: JSON.parse(backend.shop)
    readonly property var picks: list.picks ? list.picks : []
    readonly property var items: list.items ? list.items : []
    readonly property var choices: JSON.parse(backend.choices)
    readonly property var statusText: ({
        enough: i18n("Have enough"),
        missing: i18n("Missing"),
        short: i18n("Short"),
        unit: i18n("Different unit in the fridge")
    })

    // Stands in for a clipboard API: QML copies through a hidden text edit.
    TextEdit { id: clip; visible: false }
    function copy() {
        clip.text = backend.shopMarkdown()
        clip.selectAll()
        clip.copy()
        applicationWindow().showPassiveNotification(i18n("Copied: paste it into a Todo note"))
    }

    FileDialog {
        id: saveDialog
        title: i18n("Save the Shopping List")
        fileMode: FileDialog.SaveFile
        currentFile: "file://" + "shopping-list.md"
        nameFilters: [i18n("Markdown (*.md)")]
        onAccepted: page.backend.shopSave(selectedFile.toString())
    }

    Kirigami.Dialog {
        id: addDialog
        title: i18n("Add Recipe")
        standardButtons: Kirigami.Dialog.NoButton
        customFooterActions: [
            Kirigami.Action {
                text: i18n("Add")
                icon.name: "list-add"
                enabled: page.choices.length > 0
                onTriggered: { page.backend.shopAdd(page.choices[pick.currentIndex].id); addDialog.close() }
            },
            Kirigami.Action { text: i18n("Cancel"); icon.name: "dialog-cancel"; onTriggered: addDialog.close() }
        ]
        QQC2.ComboBox { id: pick; model: page.choices; textRole: "title"; implicitWidth: Kirigami.Units.gridUnit * 18 }
    }

    actions: [
        Kirigami.Action {
            text: i18n("Add Recipe…")
            icon.name: "list-add"
            shortcut: StandardKey.New
            onTriggered: addDialog.open()
        },
        Kirigami.Action {
            text: i18n("Add Planned Meals")
            icon.name: "view-calendar"
            onTriggered: page.backend.shopPlanned()
        },
        Kirigami.Action {
            text: i18n("Copy")
            icon.name: "edit-copy"
            enabled: page.items.length > 0
            shortcut: StandardKey.Copy
            onTriggered: page.copy()
        },
        Kirigami.Action {
            text: i18n("Save As…")
            icon.name: "document-save-as"
            enabled: page.items.length > 0
            onTriggered: saveDialog.open()
        },
        Kirigami.Action {
            text: i18n("Clear")
            icon.name: "edit-clear-all"
            enabled: page.picks.length > 0
            onTriggered: page.backend.shopClear()
        }
    ]

    ColumnLayout {
        spacing: 0

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: page.list.canMake === true
            type: Kirigami.MessageType.Positive
            text: i18n("You have everything for these recipes.")
        }

        Kirigami.Heading { Layout.margins: Kirigami.Units.largeSpacing; level: 3; text: i18n("Recipes"); visible: page.picks.length > 0 }
        Repeater {
            model: page.picks
            delegate: QQC2.ItemDelegate {
                id: pickRow
                required property var modelData
                Layout.fillWidth: true
                onClicked: applicationWindow().openRecipe(modelData.id)
                contentItem: RowLayout {
                    QQC2.Label { Layout.fillWidth: true; text: pickRow.modelData.title; elide: Text.ElideRight }
                    QQC2.Label { text: i18n("portions"); color: Kirigami.Theme.disabledTextColor }
                    QQC2.SpinBox {
                        from: 1
                        to: 999
                        editable: true
                        value: parseInt(pickRow.modelData.portions)
                        onValueModified: page.backend.shopPortions(pickRow.modelData.id, value)
                    }
                    QQC2.ToolButton {
                        icon.name: "list-remove"
                        text: i18n("Remove")
                        display: QQC2.AbstractButton.IconOnly
                        onClicked: page.backend.shopRemove(pickRow.modelData.id)
                        QQC2.ToolTip.text: text
                        QQC2.ToolTip.visible: hovered
                    }
                }
            }
        }

        Kirigami.Heading { Layout.margins: Kirigami.Units.largeSpacing; level: 3; text: i18n("Ingredients"); visible: page.items.length > 0 }
        Repeater {
            model: page.items
            delegate: QQC2.ItemDelegate {
                id: itemRow
                required property var modelData
                Layout.fillWidth: true
                contentItem: RowLayout {
                    Kirigami.Icon {
                        source: itemRow.modelData.status === "enough" ? "emblem-ok-symbolic" : "emblem-important-symbolic"
                        implicitWidth: Kirigami.Units.iconSizes.small
                        implicitHeight: implicitWidth
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0
                        QQC2.Label { Layout.fillWidth: true; text: itemRow.modelData.name + (itemRow.modelData.optional ? i18n(" (optional)") : ""); elide: Text.ElideRight }
                        QQC2.Label { text: page.statusText[itemRow.modelData.status]; color: Kirigami.Theme.disabledTextColor; font.pointSize: Kirigami.Theme.smallFont.pointSize }
                    }
                    QQC2.Label {
                        visible: itemRow.modelData.status !== "enough"
                        text: itemRow.modelData.get !== "" ? i18n("Get %1", itemRow.modelData.get) : i18n("Get some")
                    }
                    QQC2.Label { visible: itemRow.modelData.need !== ""; text: i18n("need %1", itemRow.modelData.need); color: Kirigami.Theme.disabledTextColor }
                }
            }
        }
    }

    EmptyState {
        visible: page.picks.length === 0
        icon.name: "view-task"
        text: i18n("Nothing on the list")
        explanation: i18n("Pick recipes, or add what is planned, and see what is missing from the fridge.")
        helpfulAction: Kirigami.Action { text: i18n("Add Recipe…"); icon.name: "list-add"; onTriggered: addDialog.open() }
    }
}
