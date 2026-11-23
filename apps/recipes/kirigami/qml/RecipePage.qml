import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard
import org.organiser.recipes

/// One recipe, pushed beside the list. Reading by default; "Edit" turns the text into fields.
/// Every field saves when it is left.
Kirigami.ScrollablePage {
    id: page
    required property var backend
    required property int recipeId
    property bool editing: false
    // The recipe, read again whenever the backend re-publishes.
    readonly property var rec: { backend.revision; return JSON.parse(backend.recipe(recipeId)) }
    readonly property bool found: rec.id !== undefined
    // Where a picture goes: a step's id, or -1 for the recipe itself.
    property int photoTarget: -1

    title: found ? rec.title : ""
    readonly property var presets: JSON.parse(backend.colorPresets())
    // The recipe's colour tints the page, as a note's does.
    background: Rectangle {
        color: page.rec.color ? Qt.alpha(page.rec.color, 0.18) : Kirigami.Theme.backgroundColor
    }

    function stars(n) { return "★".repeat(n) + "☆".repeat(5 - n) }
    onFoundChanged: if (!found) applicationWindow().pageStack.pop(page)

    FileDialog {
        id: photoDialog
        title: i18n("Add Photos")
        fileMode: FileDialog.OpenFiles
        nameFilters: [i18n("Pictures (*.jpg *.jpeg *.png *.webp *.gif *.bmp)")]
        onAccepted: page.backend.addPhotos(page.recipeId, page.photoTarget, selectedFiles.join("\n"))
    }
    function addPhotos(target) { photoTarget = target; photoDialog.open() }

    Kirigami.PromptDialog {
        id: confirmDelete
        title: i18n("Delete Recipe")
        subtitle: i18n("Delete “%1”, with its ingredients and steps?", page.title)
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: i18n("Delete")
                icon.name: "edit-delete"
                onTriggered: { confirmDelete.close(); page.backend.deleteRecipe(page.recipeId) }
            }
        ]
    }

    actions: [
        Kirigami.Action {
            text: page.editing ? i18n("Done") : i18n("Edit")
            icon.name: page.editing ? "dialog-ok-apply" : "document-edit"
            checkable: true
            checked: page.editing
            onTriggered: page.editing = !page.editing
        },
        Kirigami.Action {
            text: i18n("Favorite")
            icon.name: page.rec.favorite ? "starred-symbolic" : "non-starred-symbolic"
            onTriggered: page.backend.setFavorite(page.recipeId, !page.rec.favorite)
        },
        Kirigami.Action {
            text: i18n("Cooked Today")
            icon.name: "checkmark"
            onTriggered: page.backend.cookedToday(page.recipeId)
        },
        Kirigami.Action {
            text: i18n("Plan Meal…")
            icon.name: "view-calendar"
            onTriggered: applicationWindow().logMeal("", page.recipeId)
        },
        Kirigami.Action {
            text: i18n("Add to Shopping List")
            icon.name: "view-task"
            onTriggered: page.backend.shopAdd(page.recipeId)
        },
        Kirigami.Action {
            text: i18n("Delete Recipe…")
            icon.name: "edit-delete"
            displayHint: Kirigami.DisplayHint.AlwaysHide
            onTriggered: confirmDelete.open()
        }
    ]

    ColumnLayout {
        spacing: 0
        visible: page.found

        // ---- the recipe itself ----
        FormCard.FormHeader { title: i18n("Recipe") }
        FormCard.FormCard {
            FormCard.FormTextFieldDelegate {
                id: titleField
                visible: page.editing
                label: i18n("Title")
                onEditingFinished: if (text !== page.rec.title) page.backend.editTitle(page.recipeId, text)
                Binding on text { value: page.rec.title; when: !titleField.fieldActiveFocus }
            }
            FormCard.FormTextFieldDelegate {
                id: servingsField
                label: i18n("Serves")
                readOnly: !page.editing
                onEditingFinished: if (text !== page.rec.servings) page.backend.editServings(page.recipeId, text)
                Binding on text { value: page.rec.servings; when: !servingsField.fieldActiveFocus }
            }
            FormCard.AbstractFormDelegate {
                text: i18n("Rating")
                contentItem: RowLayout {
                    QQC2.Label { Layout.fillWidth: true; text: i18n("Rating") }
                    Repeater {
                        model: 5
                        delegate: QQC2.ToolButton {
                            required property int index
                            enabled: page.editing
                            text: index < page.rec.rating ? "★" : "☆"
                            // The star already chosen clears the rating.
                            onClicked: page.backend.setRating(page.recipeId, page.rec.rating === index + 1 ? 0 : index + 1)
                        }
                    }
                }
            }
            FormCard.AbstractFormDelegate {
                text: i18n("Color")
                background: null
                contentItem: RowLayout {
                    spacing: Kirigami.Units.largeSpacing
                    QQC2.Label { text: i18n("Color"); Layout.alignment: Qt.AlignTop }
                    ColorPicker {
                        Layout.fillWidth: true
                        presets: page.presets
                        color: page.rec.color ? page.rec.color : ""
                        onPicked: hex => page.backend.setRecipeColor(page.recipeId, hex)
                    }
                }
            }
            FormCard.FormTextFieldDelegate {
                id: tagsField
                label: i18n("Tags")
                placeholderText: i18n("Comma separated")
                readOnly: !page.editing
                onEditingFinished: if (text !== page.rec.tags.join(", ")) page.backend.setRecipeTags(page.recipeId, text)
                Binding on text { value: page.rec.tags.join(", "); when: !tagsField.fieldActiveFocus }
            }
        }

        // ---- photos ----
        FormCard.FormHeader { title: i18n("Photos"); visible: page.editing || page.rec.photos.length > 0 }
        Flow {
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.largeSpacing
            visible: page.rec.photos.length > 0
            spacing: Kirigami.Units.largeSpacing
            Repeater {
                model: page.rec.photos
                delegate: PhotoTile { target: -1 }
            }
        }
        FormCard.FormCard {
            visible: page.editing
            FormCard.FormButtonDelegate {
                text: i18n("Add Photos…")
                icon.name: "insert-image"
                onClicked: page.addPhotos(-1)
            }
        }

        // ---- ingredients ----
        FormCard.FormHeader { title: i18n("Ingredients") }
        FormCard.FormCard {
            Repeater {
                model: page.rec.ingredients
                delegate: ColumnLayout {
                    id: ing
                    required property var modelData
                    required property int index
                    spacing: 0
                    Layout.fillWidth: true
                    FormCard.FormTextDelegate {
                        visible: !page.editing
                        text: (ing.modelData.qty !== "" ? ing.modelData.qty + " " : "") + (ing.modelData.unit !== "" ? ing.modelData.unit + " " : "") + ing.modelData.name
                        description: ing.modelData.optional ? i18n("Optional") : ""
                    }
                    FormCard.AbstractFormDelegate {
                        visible: page.editing
                        background: null
                        contentItem: RowLayout {
                            QQC2.TextField { id: qty; Layout.preferredWidth: Kirigami.Units.gridUnit * 4; placeholderText: i18n("Qty"); text: ing.modelData.qty; onEditingFinished: save() }
                            QQC2.TextField { id: unit; Layout.preferredWidth: Kirigami.Units.gridUnit * 4; placeholderText: i18n("Unit"); text: ing.modelData.unit; onEditingFinished: save() }
                            QQC2.TextField { id: name; Layout.fillWidth: true; placeholderText: i18n("Ingredient"); text: ing.modelData.name; onEditingFinished: save() }
                            QQC2.CheckBox { id: opt; text: i18n("Optional"); checked: ing.modelData.optional; onToggled: save() }
                            QQC2.ToolButton { icon.name: "go-up"; enabled: ing.index > 0; onClicked: page.backend.moveIngredient(ing.modelData.id, -1); QQC2.ToolTip.text: i18n("Move Up"); QQC2.ToolTip.visible: hovered }
                            QQC2.ToolButton { icon.name: "go-down"; enabled: ing.index < page.rec.ingredients.length - 1; onClicked: page.backend.moveIngredient(ing.modelData.id, 1); QQC2.ToolTip.text: i18n("Move Down"); QQC2.ToolTip.visible: hovered }
                            QQC2.ToolButton { icon.name: "edit-delete"; onClicked: page.backend.deleteIngredient(ing.modelData.id); QQC2.ToolTip.text: i18n("Delete"); QQC2.ToolTip.visible: hovered }
                        }
                        function save() {
                            if (name.text.trim() === "") return
                            if (name.text !== ing.modelData.name || qty.text !== ing.modelData.qty || unit.text !== ing.modelData.unit || opt.checked !== ing.modelData.optional)
                                page.backend.editIngredient(ing.modelData.id, name.text, qty.text, unit.text, opt.checked)
                        }
                    }
                }
            }
            FormCard.AbstractFormDelegate {
                visible: page.editing
                background: null
                contentItem: RowLayout {
                    QQC2.TextField { id: newQty; Layout.preferredWidth: Kirigami.Units.gridUnit * 4; placeholderText: i18n("Qty") }
                    QQC2.TextField { id: newUnit; Layout.preferredWidth: Kirigami.Units.gridUnit * 4; placeholderText: i18n("Unit") }
                    QQC2.TextField { id: newName; Layout.fillWidth: true; placeholderText: i18n("New ingredient"); onAccepted: addIng.clicked() }
                    QQC2.CheckBox { id: newOpt; text: i18n("Optional") }
                    QQC2.Button {
                        id: addIng
                        icon.name: "list-add"
                        text: i18n("Add")
                        enabled: newName.text.trim() !== ""
                        onClicked: {
                            page.backend.addIngredient(page.recipeId, newName.text, newQty.text, newUnit.text, newOpt.checked)
                            newName.text = ""; newQty.text = ""; newUnit.text = ""; newOpt.checked = false
                            newQty.forceActiveFocus()
                        }
                    }
                }
            }
            FormCard.FormTextDelegate {
                visible: !page.editing && page.rec.ingredients.length === 0
                text: i18n("No ingredients yet")
            }
        }

        // ---- steps ----
        FormCard.FormHeader { title: i18n("Steps") }
        Repeater {
            model: page.rec.steps
            delegate: FormCard.FormCard {
                id: step
                required property var modelData
                required property int index
                Layout.bottomMargin: Kirigami.Units.largeSpacing
                FormCard.FormTextDelegate {
                    visible: !page.editing
                    text: i18n("Step %1", step.index + 1)
                    description: step.modelData.text
                    descriptionItem.textFormat: Text.MarkdownText
                    descriptionItem.wrapMode: Text.Wrap
                }
                FormCard.FormTextAreaDelegate {
                    id: stepText
                    visible: page.editing
                    label: i18n("Step %1", step.index + 1)
                    onEditingFinished: if (text !== step.modelData.text) page.backend.editStep(step.modelData.id, text)
                    text: step.modelData.text
                }
                Flow {
                    Layout.fillWidth: true
                    Layout.margins: Kirigami.Units.largeSpacing
                    visible: step.modelData.photos.length > 0
                    spacing: Kirigami.Units.largeSpacing
                    Repeater {
                        model: step.modelData.photos
                        delegate: PhotoTile { target: step.modelData.id }
                    }
                }
                FormCard.AbstractFormDelegate {
                    visible: page.editing
                    background: null
                    contentItem: RowLayout {
                        QQC2.Button { icon.name: "insert-image"; text: i18n("Add Photos…"); onClicked: page.addPhotos(step.modelData.id) }
                        Item { Layout.fillWidth: true }
                        QQC2.ToolButton { icon.name: "go-up"; enabled: step.index > 0; onClicked: page.backend.moveStep(step.modelData.id, -1); QQC2.ToolTip.text: i18n("Move Up"); QQC2.ToolTip.visible: hovered }
                        QQC2.ToolButton { icon.name: "go-down"; enabled: step.index < page.rec.steps.length - 1; onClicked: page.backend.moveStep(step.modelData.id, 1); QQC2.ToolTip.text: i18n("Move Down"); QQC2.ToolTip.visible: hovered }
                        QQC2.ToolButton { icon.name: "edit-delete"; onClicked: page.backend.deleteStep(step.modelData.id); QQC2.ToolTip.text: i18n("Delete Step"); QQC2.ToolTip.visible: hovered }
                    }
                }
            }
        }
        FormCard.FormCard {
            visible: page.editing
            FormCard.FormButtonDelegate {
                text: i18n("Add Step")
                icon.name: "list-add"
                onClicked: page.backend.addStep(page.recipeId)
            }
        }

        // ---- notes ----
        FormCard.FormHeader { title: i18n("Notes"); visible: page.editing || page.rec.notes !== "" }
        FormCard.FormCard {
            visible: page.editing || page.rec.notes !== ""
            FormCard.FormTextDelegate {
                visible: !page.editing
                text: page.rec.notes
                textItem.textFormat: Text.MarkdownText
                textItem.wrapMode: Text.Wrap
                onLinkActivated: link => Qt.openUrlExternally(link)
            }
            FormCard.FormTextAreaDelegate {
                id: notesField
                visible: page.editing
                label: i18n("Notes")
                onEditingFinished: if (text !== page.rec.notes) page.backend.editNotes(page.recipeId, text)
                Binding on text { value: page.rec.notes; when: !notesField.fieldActiveFocus }
            }
        }

        // ---- when it was cooked ----
        FormCard.FormHeader { title: i18n("History"); visible: page.rec.history.length > 0 }
        FormCard.FormCard {
            visible: page.rec.history.length > 0
            Repeater {
                model: page.rec.history
                delegate: FormCard.FormTextDelegate {
                    required property var modelData
                    text: applicationWindow().dates.show(modelData.date) + (modelData.timeLabel !== "" ? ", " + modelData.timeLabel : "")
                    description: (modelData.planned ? i18n("Planned") : i18n("Cooked")) + " · " + i18n("serves %1", modelData.servings)
                }
            }
        }
        Item { Layout.preferredHeight: Kirigami.Units.gridUnit }
    }

    /// A picture with a button to remove it, in edit mode.
    component PhotoTile: Item {
        id: tile
        required property var modelData
        property int target: -1
        implicitWidth: Kirigami.Units.gridUnit * 9
        implicitHeight: Kirigami.Units.gridUnit * 7
        Image {
            anchors.fill: parent
            source: tile.modelData.url
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
        }
        QQC2.Button {
            visible: page.editing
            anchors { top: parent.top; right: parent.right; margins: Kirigami.Units.smallSpacing }
            icon.name: "edit-delete"
            onClicked: page.backend.removePhoto(page.recipeId, tile.target, tile.modelData.name)
            QQC2.ToolTip.text: i18n("Remove Photo")
            QQC2.ToolTip.visible: hovered
        }
    }
}
