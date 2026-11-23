import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

/// The apps that have offered stickies, each with a switch.
FormCard.FormCardPage {
    id: page
    required property var backend
    title: i18n("Linked Apps")
    readonly property var links: JSON.parse(backend.links)

    FormCard.FormHeader { title: i18n("Apps") }
    FormCard.FormCard {
        FormCard.FormTextDelegate {
            visible: page.links.length === 0
            text: i18n("No app has offered stickies yet")
            description: i18n("Open Notes or Todo once and it will appear here.")
        }
        Repeater {
            model: page.links
            delegate: FormCard.FormSwitchDelegate {
                required property var modelData
                text: modelData.title
                description: modelData.dbPath
                checked: modelData.enabled
                onToggled: page.backend.setLinked(modelData.appId, checked)
            }
        }
    }
}
