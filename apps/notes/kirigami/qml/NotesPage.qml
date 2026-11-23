import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

/// The notes of the current scope: pinned, grouped by folder or listed, as cards or rows.
Kirigami.ScrollablePage {
    id: page
    required property var backend
    signal openNote(int id)
    signal newNote()
    signal newFolder(int parent)
    signal openFolderSettings(int id)
    signal showScope(string name, int folder)

    readonly property var pg: JSON.parse(backend.page)
    readonly property var prefs: JSON.parse(backend.prefs)
    readonly property var filterState: JSON.parse(backend.filters)
    readonly property bool listView: prefs.listView === true
    readonly property bool inFolder: pg.scope === "folder"
    readonly property bool inTrash: pg.scope === "trash"

    title: {
        if (pg.header) return pg.header.title
        switch (pg.scope) {
        case "favorites": return i18n("Favorites")
        case "archive": return i18n("Archive")
        case "trash": return i18n("Trash")
        default: return i18n("All Notes")
        }
    }

    function sortAction(name, label) {
        return { name: name, label: label }
    }

    actions: [
        Kirigami.Action {
            text: i18n("New Note")
            icon.name: "document-new"
            shortcut: StandardKey.New
            visible: !page.inTrash
            onTriggered: page.newNote()
        },
        Kirigami.Action {
            id: searchAction
            text: i18n("Search")
            icon.name: "edit-find"
            shortcut: StandardKey.Find
            checkable: true
            onTriggered: {
                if (checked) searchField.forceActiveFocus()
                else page.backend.setSearch("")
            }
        },
        Kirigami.Action {
            text: page.filterState.count > 0 ? i18n("Filter (%1)", page.filterState.count) : i18n("Filter")
            icon.name: "view-filter"
            onTriggered: filterSheet.open()
        },
        Kirigami.Action {
            text: i18n("Empty Trash")
            icon.name: "trash-empty"
            visible: page.inTrash && page.pg.count > 0
            onTriggered: emptyTrashDialog.open()
        },
        Kirigami.Action {
            text: i18n("Configure Folder…")
            icon.name: "configure"
            visible: page.inFolder
            onTriggered: page.openFolderSettings(page.pg.folderId)
        },
        Kirigami.Action {
            text: i18n("New Folder Here")
            icon.name: "folder-new"
            visible: page.inFolder && !(page.pg.header && page.pg.header.isDefault)
            onTriggered: page.newFolder(page.pg.folderId)
        },
        Kirigami.Action {
            text: i18n("Sort By")
            icon.name: "view-sort"
            Kirigami.Action {
                text: i18n("Recently Changed")
                checkable: true
                checked: page.backend.sort === "recent"
                onTriggered: page.backend.sortBy("recent")
            }
            Kirigami.Action {
                text: i18n("Newest")
                checkable: true
                checked: page.backend.sort === "created"
                onTriggered: page.backend.sortBy("created")
            }
            Kirigami.Action {
                text: i18n("Title")
                checkable: true
                checked: page.backend.sort === "title"
                onTriggered: page.backend.sortBy("title")
            }
            Kirigami.Action {
                text: i18n("Best Rated")
                checkable: true
                checked: page.backend.sort === "rating"
                onTriggered: page.backend.sortBy("rating")
            }
        },
        Kirigami.Action {
            text: i18n("View")
            icon.name: "view-list-icons"
            Kirigami.Action {
                text: i18n("Show as List")
                checkable: true
                checked: page.listView
                onTriggered: page.backend.setPref("listView", checked)
            }
            Kirigami.Action {
                text: i18n("Group by Folder")
                checkable: true
                checked: page.prefs.groupByFolder === true
                visible: page.pg.canGroup === true
                onTriggered: page.backend.setPref("groupByFolder", checked)
            }
            Kirigami.Action {
                text: i18n("Group Folder Notes by Subfolder")
                checkable: true
                checked: page.prefs.groupBySection === true
                visible: page.inFolder
                onTriggered: page.backend.setPref("groupBySection", checked)
            }
            Kirigami.Action {
                text: i18n("Show Notes of Subfolders")
                checkable: true
                checked: page.prefs.showAllNotes === true
                visible: page.inFolder
                onTriggered: page.backend.setPref("showAllNotes", checked)
            }
        }
    ]

    header: QQC2.ToolBar {
        visible: searchAction.checked
        height: visible ? implicitHeight : 0
        contentItem: Kirigami.SearchField {
            id: searchField
            onTextChanged: page.backend.setSearch(text)
            onAccepted: page.backend.setSearch(text)
        }
    }

    Keys.onEscapePressed: if (searchAction.checked) { searchField.text = ""; searchAction.checked = false }

    FilterSheet {
        id: filterSheet
        backend: page.backend
    }

    Kirigami.PromptDialog {
        id: emptyTrashDialog
        title: i18n("Empty Trash")
        subtitle: i18n("Every note in the trash will be deleted for good.")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: page.backend.emptyTrash()
    }

    /// A block of notes: cards, or rows when the list view is on.
    component NoteBlock: ColumnLayout {
        id: block
        property var items: []
        spacing: Kirigami.Units.smallSpacing
        // Cards flow left to right and wrap: as many columns as fit, each sharing the width, so a
        // wide window shows several notes side by side and a narrow pane shows one.
        Flow {
            id: flow
            visible: !page.listView
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing
            readonly property real minCardWidth: Kirigami.Units.gridUnit * 18
            readonly property int columns: Math.max(1, Math.floor((width + spacing) / (minCardWidth + spacing)))
            readonly property real cardWidth: Math.max(0, (width - spacing * (columns - 1)) / columns)
            Repeater {
                model: block.items
                delegate: NoteCard {
                    required property var modelData
                    width: flow.cardWidth
                    note: modelData
                    onOpened: page.openNote(modelData.id)
                }
            }
        }
        // A real ListView: Kirigami's SwipeListItem looks up through its parents for the
        // ListView it is a row of, and fails (a TypeError per row) when built by a Repeater. The
        // page scrolls, so this one does not.
        ListView {
            visible: page.listView
            Layout.fillWidth: true
            implicitHeight: contentHeight
            interactive: false
            model: block.items
            delegate: NoteRow {
                required property var modelData
                width: ListView.view.width
                note: modelData
                backend: page.backend
                onOpened: page.openNote(modelData.id)
            }
        }
    }

    /// A folder to open, as a card.
    component FolderCard: Kirigami.AbstractCard {
        id: fc
        required property var folder
        showClickFeedback: true
        onClicked: page.showScope("folder", folder.id)
        contentItem: RowLayout {
            spacing: Kirigami.Units.largeSpacing
            Kirigami.Icon {
                source: fc.folder.icon
                color: fc.folder.color !== "" ? fc.folder.color : Kirigami.Theme.textColor
                Layout.preferredWidth: Kirigami.Units.iconSizes.medium
                Layout.preferredHeight: Kirigami.Units.iconSizes.medium
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 0
                Kirigami.Heading { level: 4; text: fc.folder.title; Layout.fillWidth: true; elide: Text.ElideRight }
                QQC2.Label {
                    Layout.fillWidth: true
                    text: fc.folder.notes !== "" ? fc.folder.notes : i18np("%1 note", "%1 notes", fc.folder.total)
                    elide: Text.ElideRight
                    color: Kirigami.Theme.disabledTextColor
                }
            }
        }
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        // Where this folder is: the folders above it.
        Flow {
            Layout.fillWidth: true
            visible: page.inFolder && page.pg.header !== null
            spacing: Kirigami.Units.smallSpacing
            Kirigami.LinkButton {
                text: i18n("All Notes")
                onClicked: page.showScope("notes", -1)
            }
            Repeater {
                model: page.pg.header ? page.pg.header.path : []
                delegate: RowLayout {
                    required property var modelData
                    required property int index
                    spacing: Kirigami.Units.smallSpacing
                    QQC2.Label { text: "›" }
                    Kirigami.LinkButton {
                        enabled: index < (page.pg.header.path.length - 1)
                        text: modelData.title
                        onClicked: page.showScope("folder", modelData.id)
                    }
                }
            }
        }

        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: page.pg.trashNotice !== null && page.pg.trashNotice !== undefined
            text: page.pg.trashNotice ? page.pg.trashNotice : ""
        }

        QQC2.Label {
            Layout.fillWidth: true
            visible: page.inFolder && page.pg.header !== null && page.pg.header.notes !== ""
            text: page.pg.header ? page.pg.header.notes : ""
            wrapMode: Text.Wrap
            color: Kirigami.Theme.disabledTextColor
        }

        // Sub-folders as cards to open.
        Kirigami.CardsLayout {
            Layout.fillWidth: true
            visible: page.pg.cards.length > 0
            Repeater {
                model: page.pg.cards
                delegate: FolderCard { required property var modelData; folder: modelData }
            }
        }

        // The notes that are not in a group.
        Kirigami.ListSectionHeader {
            Layout.fillWidth: true
            visible: page.pg.pinned.length > 0
            label: i18n("Pinned")
        }
        NoteBlock { Layout.fillWidth: true; items: page.pg.pinned; visible: items.length > 0 }
        Kirigami.ListSectionHeader {
            Layout.fillWidth: true
            visible: page.pg.pinned.length > 0 && page.pg.notes.length > 0 && page.pg.groups.length === 0
            label: i18n("Others")
        }
        NoteBlock { Layout.fillWidth: true; items: page.pg.notes; visible: items.length > 0 }

        // One group per folder.
        Repeater {
            model: page.pg.groups
            delegate: ColumnLayout {
                id: group
                required property var modelData
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing
                RowLayout {
                    Layout.fillWidth: true
                    Kirigami.Icon {
                        source: group.modelData.folder.icon
                        color: group.modelData.folder.color !== "" ? group.modelData.folder.color : Kirigami.Theme.textColor
                        Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
                        Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
                    }
                    Kirigami.Heading {
                        level: 3
                        text: group.modelData.folder.title
                    }
                    QQC2.Label {
                        text: i18np("%1 note", "%1 notes", group.modelData.count)
                        color: Kirigami.Theme.disabledTextColor
                    }
                    Item { Layout.fillWidth: true }
                    QQC2.ToolButton {
                        icon.name: "configure"
                        text: i18n("Configure Folder…")
                        display: QQC2.AbstractButton.IconOnly
                        QQC2.ToolTip.text: text
                        QQC2.ToolTip.visible: hovered
                        onClicked: page.openFolderSettings(group.modelData.folder.id)
                    }
                    QQC2.ToolButton {
                        icon.name: "go-next"
                        text: i18n("Open Folder")
                        display: QQC2.AbstractButton.IconOnly
                        QQC2.ToolTip.text: text
                        QQC2.ToolTip.visible: hovered
                        onClicked: page.showScope("folder", group.modelData.folder.id)
                    }
                }
                Kirigami.Separator { Layout.fillWidth: true }
                Kirigami.CardsLayout {
                    Layout.fillWidth: true
                    visible: group.modelData.cards.length > 0
                    Repeater {
                        model: group.modelData.cards
                        delegate: FolderCard { required property var modelData; folder: modelData }
                    }
                }
                NoteBlock { Layout.fillWidth: true; items: group.modelData.notes; visible: items.length > 0 }
                QQC2.Label {
                    visible: group.modelData.notes.length === 0 && group.modelData.cards.length === 0
                    text: i18n("No notes in this folder")
                    color: Kirigami.Theme.disabledTextColor
                }
            }
        }

        Kirigami.PlaceholderMessage {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: page.pg.empty !== null && page.pg.empty !== undefined
            icon.name: page.pg.empty ? page.pg.empty.icon : ""
            text: page.pg.empty ? page.pg.empty.title : ""
            explanation: page.pg.empty ? page.pg.empty.hint : ""
        }
    }
}
