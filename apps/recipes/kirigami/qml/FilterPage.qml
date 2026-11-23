import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard
import org.organiser.recipes

/// What the recipe list shows: favourites, a least rating, and tags (a recipe needs every tag picked).
Kirigami.ScrollablePage {
    id: page
    required property var backend
    title: i18n("Filter")
    readonly property var tags: JSON.parse(backend.tags)
    readonly property var picked: backend.tagFilter === "" ? [] : backend.tagFilter.split(",")

    function toggleTag(name, on) {
        const list = picked.filter(t => t !== name)
        if (on) list.push(name)
        backend.tagFilter = list.join(",")
        backend.applyFilters()
    }

    actions: Kirigami.Action {
        text: i18n("Clear Filters")
        icon.name: "edit-clear-all"
        enabled: page.backend.filterCount > 0
        onTriggered: page.backend.clearFilters()
    }

    ColumnLayout {
        spacing: 0
        FormCard.FormHeader { title: i18n("Recipes") }
        FormCard.FormCard {
            FormCard.FormSwitchDelegate {
                text: i18n("Favorites only")
                checked: page.backend.favoritesOnly
                onToggled: { page.backend.favoritesOnly = checked; page.backend.applyFilters() }
            }
            FormCard.FormComboBoxDelegate {
                text: i18n("Rated at least")
                model: [i18n("Any"), "★", "★★", "★★★", "★★★★", "★★★★★"]
                currentIndex: page.backend.minRating
                onActivated: { page.backend.minRating = currentIndex; page.backend.applyFilters() }
            }
        }
        FormCard.FormHeader { title: i18n("Tags") }
        FormCard.FormCard {
            Repeater {
                model: page.tags
                delegate: FormCard.FormCheckDelegate {
                    required property var modelData
                    text: modelData.parent ? modelData.parent + " / " + modelData.name : modelData.name
                    checked: page.picked.indexOf(modelData.name) >= 0
                    onToggled: page.toggleTag(modelData.name, checked)
                }
            }
            FormCard.FormTextDelegate {
                visible: page.tags.length === 0
                text: i18n("No tags yet")
                description: i18n("Add tags to a recipe from its page.")
            }
        }
    }
}
