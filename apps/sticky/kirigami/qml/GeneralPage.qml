import QtQuick
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

FormCard.FormCardPage {
    id: page
    required property var backend
    title: i18n("General")

    FormCard.FormHeader { title: i18n("Startup") }
    FormCard.FormCard {
        FormCard.FormSwitchDelegate {
            text: i18n("Start on login")
            description: i18n("Show your stickies when you log in. Sticky Notes ends by itself if there are none.")
            checked: page.backend.autostart
            onToggled: page.backend.enableAutostart(checked)
        }
    }
}
