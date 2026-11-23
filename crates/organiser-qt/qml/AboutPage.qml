import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

/// The about page, from the app's metadata. Our own page rather than `Kirigami.AboutPage`, whose
/// licence sheet has a binding loop (a warning every time it opens, in every app).
Kirigami.ScrollablePage {
    id: page
    required property string appName
    required property string appComment
    property string appId: ""
    readonly property string version: "0.0.3"
    readonly property string homepage: "https://github.com/VaZark"

    title: i18n("About %1", appName)

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        Kirigami.Icon {
            source: page.appId
            Layout.alignment: Qt.AlignHCenter
            Layout.topMargin: Kirigami.Units.gridUnit
            Layout.preferredWidth: Kirigami.Units.iconSizes.huge
            Layout.preferredHeight: Kirigami.Units.iconSizes.huge
        }
        Kirigami.Heading {
            Layout.alignment: Qt.AlignHCenter
            level: 1
            text: page.appName
        }
        QQC2.Label {
            Layout.alignment: Qt.AlignHCenter
            text: i18n("Version %1", page.version)
            color: Kirigami.Theme.disabledTextColor
        }
        QQC2.Label {
            Layout.fillWidth: true
            Layout.maximumWidth: Kirigami.Units.gridUnit * 30
            Layout.alignment: Qt.AlignHCenter
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            visible: text !== ""
            text: page.appComment
        }
        Kirigami.UrlButton {
            Layout.alignment: Qt.AlignHCenter
            url: page.homepage
        }
        QQC2.Label {
            Layout.alignment: Qt.AlignHCenter
            text: i18n("Licensed under the MIT License")
        }
        QQC2.Label {
            Layout.alignment: Qt.AlignHCenter
            text: "© Vasanth M"
            color: Kirigami.Theme.disabledTextColor
        }
    }
}
