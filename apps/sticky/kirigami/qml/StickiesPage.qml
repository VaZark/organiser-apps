import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

/// Every sticky as a card, tinted with its paper. A card brings its sticky forward.
Kirigami.ScrollablePage {
    id: page
    required property var backend
    /// A card was chosen: bring its sticky to the desktop.
    signal showSticky(string key)

    title: i18n("All Notes")
    readonly property var stickies: JSON.parse(backend.stickies)
    readonly property var links: JSON.parse(backend.links)

    actions: [
        Kirigami.Action {
            text: i18n("New Note in…")
            icon.name: "list-add"
            enabled: page.links.some(l => l.enabled)
            onTriggered: newMenu.popup()
        }
    ]

    QQC2.Menu {
        id: newMenu
        Instantiator {
            id: appItems
            // An integer model: the entries are read by index.
            readonly property var apps: page.links.filter(l => l.enabled)
            model: apps.length
            delegate: QQC2.MenuItem {
                required property int index
                readonly property var app: appItems.apps[index]
                text: app.title
                onTriggered: page.backend.newIn(app.appId)
            }
            onObjectAdded: (i, o) => newMenu.insertItem(i, o)
            onObjectRemoved: (i, o) => newMenu.removeItem(o)
        }
    }

    Kirigami.PlaceholderMessage {
        anchors.centerIn: parent
        width: parent.width - Kirigami.Units.gridUnit * 4
        visible: page.stickies.length === 0
        icon.name: "note"
        text: i18n("No notes yet")
        explanation: page.links.length === 0
            ? i18n("Link an app in Configure Sticky Notes, then turn an item into a sticky.")
            : i18n("Turn an item of a linked app into a sticky and it appears here.")
    }

    Kirigami.CardsLayout {
        visible: page.stickies.length > 0
        maximumColumns: 4
        minimumColumnWidth: Kirigami.Units.gridUnit * 14
        Repeater {
            model: page.stickies
            delegate: Kirigami.Card {
                id: card
                required property var modelData
                Layout.alignment: Qt.AlignTop

                // The paper as the card's colour, the ink for its text.
                Kirigami.Theme.inherit: false
                Kirigami.Theme.colorSet: Kirigami.Theme.View
                Kirigami.Theme.backgroundColor: modelData.paper
                Kirigami.Theme.textColor: modelData.ink

                showClickFeedback: true
                onClicked: page.showSticky(modelData.key)

                banner.title: modelData.title ? modelData.title : i18n("Untitled")
                banner.titleLevel: 3

                contentItem: QQC2.Label {
                    text: card.modelData.body
                    textFormat: Text.MarkdownText
                    wrapMode: Text.Wrap
                    elide: Text.ElideRight
                    maximumLineCount: 8
                    color: card.modelData.ink
                }

                footer: RowLayout {
                    QQC2.Label {
                        Layout.fillWidth: true
                        text: card.modelData.appName
                        color: Qt.alpha(card.modelData.ink, 0.7)
                        font: Kirigami.Theme.smallFont
                    }
                    QQC2.ToolButton {
                        icon.name: card.modelData.desktop ? "window-unpin" : "window-pin"
                        icon.color: card.modelData.ink
                        display: QQC2.AbstractButton.IconOnly
                        text: card.modelData.desktop
                            ? i18n("On the desktop: keep it only here")
                            : i18n("Only here: show it on the desktop")
                        QQC2.ToolTip.text: text
                        QQC2.ToolTip.visible: hovered
                        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                        onClicked: page.backend.setDesktop(card.modelData.key, !card.modelData.desktop)
                    }
                }
            }
        }
    }
}
