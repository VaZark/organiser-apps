import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.todo

/// The folder of preferences: each entry opens its page on top of this one.
Kirigami.ScrollablePage {
    id: page
    required property var backend
    signal openSection(string type)
    title: i18n("Preferences")

    ListView {
        model: [
            { name: i18n("General"), hint: i18n("Lists and the tasks folder"), icon: "configure", type: "GeneralPrefsPage" },
            { name: i18n("Tags"), hint: i18n("Names, parents and colours"), icon: "tag", type: "TagsPage" },
            { name: i18n("Statuses"), hint: i18n("The board's columns and what means finished"), icon: "flag", type: "StatusesPage" }
        ]
        delegate: QQC2.ItemDelegate {
            id: d
            required property var modelData
            width: ListView.view.width
            verticalPadding: Kirigami.Units.largeSpacing
            onClicked: page.openSection(modelData.type)
            contentItem: RowLayout {
                spacing: Kirigami.Units.largeSpacing
                Kirigami.Icon { source: d.modelData.icon; Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium; Layout.preferredHeight: width }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 0
                    QQC2.Label { Layout.fillWidth: true; text: d.modelData.name }
                    QQC2.Label { Layout.fillWidth: true; text: d.modelData.hint; font: Kirigami.Theme.smallFont; color: Kirigami.Theme.disabledTextColor }
                }
                Kirigami.Icon { source: "go-next"; Layout.preferredWidth: Kirigami.Units.iconSizes.small; Layout.preferredHeight: width }
            }
        }
    }
}
