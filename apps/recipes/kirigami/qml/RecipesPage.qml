import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.recipes

/// Every recipe: cards, or one line each when "compact" is set. Opening one pushes its page.
Kirigami.ScrollablePage {
    id: page
    required property var backend
    title: i18n("Recipes")
    readonly property var recipes: JSON.parse(backend.recipes)
    readonly property var sorts: [
        { value: "manual", text: i18n("My Order") },
        { value: "title", text: i18n("Title") },
        { value: "rating", text: i18n("Best Rated") },
        { value: "recent", text: i18n("Newest") }
    ]

    function stars(n) { return "★".repeat(n) + "☆".repeat(5 - n) }
    function open(id) { applicationWindow().openRecipe(id) }

    Kirigami.PromptDialog {
        id: newRecipe
        title: i18n("New Recipe")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        QQC2.TextField {
            id: newTitle
            placeholderText: i18n("Title")
            onAccepted: newRecipe.accept()
        }
        onOpened: { newTitle.text = ""; newTitle.forceActiveFocus() }
        onAccepted: {
            const id = page.backend.createRecipe(newTitle.text)
            if (id >= 0) page.open(id)
        }
    }

    actions: [
        Kirigami.Action {
            text: i18n("New Recipe")
            icon.name: "list-add"
            shortcut: StandardKey.New
            onTriggered: newRecipe.open()
        },
        Kirigami.Action {
            text: page.backend.filterCount > 0 ? i18n("Filter (%1)…", page.backend.filterCount) : i18n("Filter…")
            icon.name: "view-filter"
            onTriggered: applicationWindow().configure("FilterPage.qml")
        },
        Kirigami.Action {
            text: i18n("Sort By")
            icon.name: "view-sort"
            Kirigami.Action {
                text: i18n("My Order")
                checkable: true
                checked: page.backend.sort === "manual"
                onTriggered: { page.backend.sort = "manual"; page.backend.applyFilters() }
            }
            Kirigami.Action {
                text: i18n("Title")
                checkable: true
                checked: page.backend.sort === "title"
                onTriggered: { page.backend.sort = "title"; page.backend.applyFilters() }
            }
            Kirigami.Action {
                text: i18n("Best Rated")
                checkable: true
                checked: page.backend.sort === "rating"
                onTriggered: { page.backend.sort = "rating"; page.backend.applyFilters() }
            }
            Kirigami.Action {
                text: i18n("Newest")
                checkable: true
                checked: page.backend.sort === "recent"
                onTriggered: { page.backend.sort = "recent"; page.backend.applyFilters() }
            }
        }
    ]

    header: QQC2.ToolBar {
        contentItem: RowLayout {
            Kirigami.SearchField {
                id: searchField
                Layout.fillWidth: true
                placeholderText: i18n("Search recipes…")
                text: page.backend.search
                onTextChanged: if (text !== page.backend.search) { page.backend.search = text; page.backend.applyFilters() }
            }
        }
    }

    Shortcut {
        sequences: [StandardKey.Find]
        onActivated: searchField.forceActiveFocus()
    }

    Component { id: lineDelegate
        Kirigami.SwipeListItem {
            id: line
            required property var modelData
            onClicked: page.open(modelData.id)
            contentItem: RowLayout {
                spacing: Kirigami.Units.largeSpacing
                Kirigami.Icon { source: line.modelData.favorite ? "starred-symbolic" : "non-starred-symbolic"; implicitWidth: Kirigami.Units.iconSizes.small; implicitHeight: implicitWidth }
                QQC2.Label { Layout.fillWidth: true; text: line.modelData.title; elide: Text.ElideRight }
                QQC2.Label { visible: line.modelData.rating > 0; text: page.stars(line.modelData.rating); color: Kirigami.Theme.disabledTextColor }
            }
            actions: [
                Kirigami.Action {
                    text: line.modelData.favorite ? i18n("Remove From Favorites") : i18n("Add to Favorites")
                    icon.name: line.modelData.favorite ? "non-starred-symbolic" : "starred-symbolic"
                    onTriggered: page.backend.setFavorite(line.modelData.id, !line.modelData.favorite)
                },
                Kirigami.Action {
                    text: i18n("Add to Shopping List")
                    icon.name: "view-task"
                    onTriggered: page.backend.shopAdd(line.modelData.id)
                }
            ]
        }
    }
    Component { id: cardDelegate
        Kirigami.Card {
            id: card
            required property var modelData
            showClickFeedback: true
            onClicked: page.open(modelData.id)
            banner.source: modelData.photo
            contentItem: ColumnLayout {
                spacing: Kirigami.Units.smallSpacing
                QQC2.Label {
                    Layout.fillWidth: true
                    text: card.modelData.title
                    font.bold: true
                    font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.2
                    wrapMode: Text.Wrap
                }
                QQC2.Label {
                    visible: card.modelData.rating > 0
                    text: page.stars(card.modelData.rating)
                    color: Kirigami.Theme.disabledTextColor
                }
                QQC2.Label {
                    visible: card.modelData.notes !== ""
                    Layout.fillWidth: true
                    text: card.modelData.notes
                    maximumLineCount: 3
                    elide: Text.ElideRight
                    wrapMode: Text.Wrap
                }
                RowLayout {
                    visible: card.modelData.tags.length > 0
                    Layout.fillWidth: true
                    Layout.minimumHeight: Kirigami.Units.gridUnit * 1.4
                    Layout.preferredHeight: Kirigami.Units.gridUnit * 1.4
                    spacing: Kirigami.Units.smallSpacing
                    Repeater {
                        model: card.modelData.tags
                        delegate: TagChip { required property string modelData; text: modelData }
                    }
                }
            }
            actions: [
                Kirigami.Action {
                    text: card.modelData.favorite ? i18n("Remove From Favorites") : i18n("Add to Favorites")
                    icon.name: card.modelData.favorite ? "starred-symbolic" : "non-starred-symbolic"
                    onTriggered: page.backend.setFavorite(card.modelData.id, !card.modelData.favorite)
                },
                Kirigami.Action {
                    text: i18n("Add to Shopping List")
                    icon.name: "view-task"
                    onTriggered: page.backend.shopAdd(card.modelData.id)
                }
            ]
        }
    }

    // Cards, or one line each.
    Kirigami.CardsListView {
        id: list
        model: page.recipes
        delegate: page.backend.compact ? lineDelegate : cardDelegate
    }

    EmptyState {
        visible: page.recipes.length === 0
        icon.name: "view-list-details"
        text: page.backend.filterCount > 0 || page.backend.search !== "" ? i18n("No matching recipes") : i18n("No recipes yet")
        helpfulAction: Kirigami.Action {
            text: i18n("New Recipe")
            icon.name: "list-add"
            onTriggered: newRecipe.open()
        }
    }
}
