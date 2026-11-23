import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard
import org.organiser.recipes

FormCard.FormCardPage {
    id: page
    required property var backend
    title: i18n("Configure Recipes")

    FormCard.FormHeader { title: i18n("Recipe List") }
    FormCard.FormCard {
        FormCard.FormSwitchDelegate {
            text: i18n("Compact lines")
            description: i18n("Show one line per recipe instead of cards.")
            checked: page.backend.compact
            onToggled: { page.backend.compact = checked; page.backend.savePreferences() }
        }
    }
    FormCard.FormHeader { title: i18n("Appearance") }
    FormCard.FormCard {
        FormCard.FormTextDelegate {
            text: i18n("Follows your KDE color scheme and style.")
        }
    }
}
