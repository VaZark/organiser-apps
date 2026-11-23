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

    // How the cards are ordered and which app's notes show; only what is drawn changes.
    property string sortKey: "recent"
    property string appFilter: ""
    readonly property var appNames: Array.from(new Set(stickies.map(s => s.appName).filter(n => n)))
    readonly property var shown: {
        const list = stickies.filter(s => page.appFilter === "" || s.appName === page.appFilter)
        if (sortKey === "title") list.sort((a, b) => (a.title || "").localeCompare(b.title || ""))
        else if (sortKey === "app") list.sort((a, b) => (a.appName || "").localeCompare(b.appName || ""))
        return list
    }

    actions: [
        Kirigami.Action {
            text: i18n("Filter by App")
            icon.name: "view-filter"
            displayHint: Kirigami.DisplayHint.IconOnly
            checkable: true
            checked: page.appFilter !== ""
            enabled: page.appNames.length > 0
            onTriggered: {
                checked = Qt.binding(() => page.appFilter !== "")
                applicationWindow().showSheet(i18n("Filter by App"), [{ text: i18n("All Apps"), checkable: true, checked: page.appFilter === "", trigger: () => page.appFilter = "" }]
                    .concat(page.appNames.map(n => ({ text: n, checkable: true, checked: page.appFilter === n, trigger: () => page.appFilter = n }))))
            }
        },
        Kirigami.Action {
            text: i18n("Sort")
            icon.name: "view-sort"
            displayHint: Kirigami.DisplayHint.IconOnly
            Kirigami.Action { text: i18n("Newest"); checkable: true; checked: page.sortKey === "recent"; onTriggered: page.sortKey = "recent" }
            Kirigami.Action { text: i18n("Title"); checkable: true; checked: page.sortKey === "title"; onTriggered: page.sortKey = "title" }
            Kirigami.Action { text: i18n("App"); checkable: true; checked: page.sortKey === "app"; onTriggered: page.sortKey = "app" }
        }
    ]

    // The bottom bar: make a note, and put every note on the desktop or take them all off.
    footer: QQC2.ToolBar {
        contentItem: RowLayout {
            QQC2.ToolButton {
                text: i18n("New Note in…")
                icon.name: "list-add"
                display: QQC2.AbstractButton.IconOnly
                QQC2.ToolTip.text: text
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                enabled: page.links.some(l => l.enabled)
                onClicked: applicationWindow().showSheet(i18n("New Note in…"), page.links.filter(l => l.enabled).map(l => ({ text: l.title, icon: { name: "list-add" }, trigger: () => page.backend.newIn(l.appId) })))
            }
            Item { Layout.fillWidth: true }
            QQC2.ToolButton {
                text: i18n("Show All on Desktop")
                icon.name: "window-pin"
                display: QQC2.AbstractButton.IconOnly
                QQC2.ToolTip.text: text
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                enabled: page.shown.some(s => !s.desktop)
                onClicked: page.shown.forEach(s => { if (!s.desktop) page.backend.setDesktop(s.key, true) })
            }
            QQC2.ToolButton {
                text: i18n("Hide All from Desktop")
                icon.name: "window-unpin"
                display: QQC2.AbstractButton.IconOnly
                QQC2.ToolTip.text: text
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                enabled: page.shown.some(s => s.desktop)
                onClicked: page.shown.forEach(s => { if (s.desktop) page.backend.setDesktop(s.key, false) })
            }
        }
    }


    Kirigami.PlaceholderMessage {
        anchors.centerIn: parent
        width: parent.width - Kirigami.Units.gridUnit * 4
        visible: page.shown.length === 0
        icon.name: "note"
        text: page.stickies.length > 0 ? i18n("No notes from this app") : i18n("No notes yet")
        explanation: page.stickies.length > 0 ? i18n("Choose All Apps in the filter to see the rest.")
            : page.links.length === 0
            ? i18n("Link an app in Configure Sticky Notes, then turn an item into a sticky.")
            : i18n("Turn an item of a linked app into a sticky and it appears here.")
    }

    Kirigami.CardsLayout {
        visible: page.shown.length > 0
        maximumColumns: 4
        minimumColumnWidth: Kirigami.Units.gridUnit * 14
        Repeater {
            model: page.shown
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
