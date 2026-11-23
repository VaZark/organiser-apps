import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.recipes

/// Put a recipe on a day: cooked (up to today) or planned (later).
Kirigami.Dialog {
    id: dialog
    required property var backend
    // The day to start on ("": today) and the recipe (-1: choose one).
    property string day: ""
    property int recipeId: -1
    readonly property var choices: JSON.parse(backend.choices)
    readonly property var times: [{ value: "", text: i18n("Any time") }].concat(JSON.parse(backend.mealTimes()))

    title: i18n("Log Meal")
    standardButtons: Kirigami.Dialog.NoButton
    customFooterActions: [
        Kirigami.Action {
            text: i18n("Log")
            icon.name: "dialog-ok"
            enabled: dialog.choices.length > 0
            onTriggered: {
                const r = dialog.choices[recipe.currentIndex]
                if (r && dialog.backend.logMeal(r.id, date.text, time.currentValue, servings.text)) dialog.close()
            }
        },
        Kirigami.Action {
            text: i18n("Cancel")
            icon.name: "dialog-cancel"
            onTriggered: dialog.close()
        }
    ]

    function servingsOf(index) {
        const c = choices[index]
        return c && c.servings !== "" ? c.servings : "1"
    }

    onAboutToShow: {
        date.text = day !== "" ? day : backend.today
        const at = choices.findIndex(c => c.id === recipeId)
        recipe.currentIndex = at >= 0 ? at : 0
        servings.text = servingsOf(recipe.currentIndex)
        time.currentIndex = 0
    }

    Kirigami.FormLayout {
        QQC2.ComboBox {
            id: recipe
            Kirigami.FormData.label: i18n("Recipe:")
            Layout.fillWidth: true
            model: dialog.choices
            textRole: "title"
            onActivated: servings.text = dialog.servingsOf(currentIndex)
        }
        QQC2.TextField {
            id: date
            Kirigami.FormData.label: i18n("Date:")
            placeholderText: "2026-11-30"
        }
        QQC2.ComboBox {
            id: time
            Kirigami.FormData.label: i18n("Time of day:")
            model: dialog.times
            textRole: "text"
            valueRole: "value"
        }
        QQC2.TextField {
            id: servings
            Kirigami.FormData.label: i18n("Servings:")
        }
    }
}
