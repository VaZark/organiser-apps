import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.notes

/// A note's details, in the column beside the list. Edits are saved as they are made. With
/// `noteId` -1 it is a note not made yet: nothing is saved until Create Note.
Kirigami.ScrollablePage {
    id: page
    required property var backend
    property int noteId: -1
    signal created(int id)

    readonly property bool isDraft: noteId < 0
    readonly property var folderList: JSON.parse(backend.folders)
    readonly property var presets: JSON.parse(backend.prefs).presets

    // What a draft holds until it is created.
    property var draft: ({ color: "", favorite: false, pinned: false, rating: 0, tags: [], folderId: defaultFolder() })
    function defaultFolder() {
        if (backend.scope === "folder") return backend.folderId
        const d = folderList.find(f => f.isDefault)
        return d ? d.id : -1
    }

    // The note as stored, re-read whenever the page's data changes.
    readonly property var stored: { backend.page; return isDraft ? ({}) : JSON.parse(backend.note(noteId)) }
    readonly property var note: isDraft ? Object.assign({ title: "", body: "", trashed: false, archived: false,
        folderId: draft.folderId, rated: (folderList.find(f => f.id === draft.folderId) || {}).rated === true }, draft) : stored
    readonly property bool trashed: note.trashed === true

    // The note colour tints the page behind the editor.
    background: Rectangle {
        color: page.note.color ? Qt.alpha(page.note.color, 0.18) : Kirigami.Theme.backgroundColor
    }

    title: isDraft ? i18n("New Note") : (note.title ? note.title : i18n("Note"))

    function update(field, value) {
        if (isDraft) {
            const d = Object.assign({}, draft)
            d[field] = value
            draft = d
        }
    }
    function setFavorite(on) { isDraft ? update("favorite", on) : backend.setFavorite(noteId, on) }
    function setPinned(on) { isDraft ? update("pinned", on) : backend.setPinned(noteId, on) }
    function setRating(n) { isDraft ? update("rating", n) : backend.setRating(noteId, n) }
    function setColor(c) { isDraft ? update("color", c) : backend.setNoteColor(noteId, c) }
    function setFolder(id) { isDraft ? update("folderId", id) : backend.moveNote(noteId, id) }
    function setTags(list) { isDraft ? update("tags", list) : backend.setNoteTags(noteId, list.join(",")) }
    function close() { applicationWindow().pageStack.removePage(page) }

    function save() {
        if (isDraft) return
        backend.editNote(noteId, titleField.text, bodyArea.text)
    }

    function create() {
        const id = backend.createNote(JSON.stringify({
            title: titleField.text, body: bodyArea.text, color: draft.color, tags: draft.tags,
            folder: draft.folderId, favorite: draft.favorite, rating: draft.rating, pinned: draft.pinned
        }))
        if (id >= 0) created(id)
    }

    Timer {
        id: saveTimer
        interval: 500
        onTriggered: page.save()
    }
    // A pending edit is not lost when the page goes.
    Component.onDestruction: if (saveTimer.running) { saveTimer.stop(); page.save() }

    // Nothing in the header but Back: every action of the note is at the foot, in the bar or in its
    // one menu, so there is a single place to look (as in Todo's task page).
    readonly property bool ownMenu: true
    readonly property bool roomy: width >= Kirigami.Units.gridUnit * 20
    Kirigami.Action {
        id: createAction
        text: i18n("Create Note")
        icon.name: "document-save"
        shortcut: StandardKey.Save
        visible: page.isDraft
        onTriggered: page.create()
    }
    Kirigami.Action {
        id: favoriteAction
        text: page.note.favorite ? i18n("Remove from Favorites") : i18n("Add to Favorites")
        icon.name: page.note.favorite ? "emblem-favorite" : "bookmark-new"
        visible: !page.trashed
        checkable: true
        checked: page.note.favorite === true
        onTriggered: page.setFavorite(!page.note.favorite)
    }
    Kirigami.Action {
        id: pinAction
        text: page.note.pinned ? i18n("Unpin") : i18n("Pin")
        icon.name: "pin"
        visible: !page.trashed
        checkable: true
        checked: page.note.pinned === true
        onTriggered: page.setPinned(!page.note.pinned)
    }
    Kirigami.Action {
        id: stickyAction
        text: page.note.sticky ? i18n("Remove Sticky Note") : i18n("Make Sticky Note")
        icon.name: "view-pim-notes"
        visible: !page.isDraft && !page.trashed
        checkable: true
        checked: page.note.sticky === true
        onTriggered: page.backend.setSticky(page.noteId, !page.note.sticky)
    }

    // The rest of the foot's actions.
    Kirigami.Action {
        id: previewAction
        text: i18n("Preview")
        icon.name: "view-preview"
        checkable: true
    }
    Kirigami.Action {
        id: archiveAction
        text: page.note.archived ? i18n("Unarchive Note") : i18n("Archive Note")
        icon.name: "folder-archive"
        visible: !page.isDraft && !page.trashed
        onTriggered: page.backend.setArchived(page.noteId, !page.note.archived)
    }
    Kirigami.Action {
        id: showFileAction
        text: i18n("Show Note File in File Manager")
        icon.name: "folder-open"
        visible: !page.isDraft && !page.trashed && page.backend.noteDir(page.noteId) !== ""
        onTriggered: Qt.openUrlExternally("file://" + page.backend.noteDir(page.noteId))
    }
    Kirigami.Action {
        id: trashAction
        text: i18n("Move to Trash")
        icon.name: "user-trash"
        shortcut: StandardKey.Delete
        visible: !page.isDraft && !page.trashed
        onTriggered: { page.backend.trashNote(page.noteId); page.close() }
    }
    Kirigami.Action {
        id: restoreAction
        text: i18n("Restore")
        icon.name: "edit-undo"
        visible: page.trashed
        onTriggered: { page.backend.restoreNote(page.noteId); page.close() }
    }
    Kirigami.Action {
        id: purgeAction
        text: i18n("Delete Permanently")
        icon.name: "edit-delete"
        visible: page.trashed
        onTriggered: purgeDialog.open()
    }

    // The foot: Preview, then up to three icons in one row while there is room (favourite, pin,
    // sticky note), and the one menu for the rest. Without room the icons go into the menu too.
    footer: QQC2.ToolBar {
        position: QQC2.ToolBar.Footer
        contentItem: RowLayout {
            spacing: Kirigami.Units.smallSpacing
            // A mode, so it says what it is.
            QQC2.ToolButton {
                action: previewAction
                display: QQC2.AbstractButton.TextBesideIcon
            }
            QQC2.ToolButton {
                visible: page.isDraft
                text: createAction.text
                icon.name: createAction.icon.name
                display: QQC2.AbstractButton.TextBesideIcon
                onClicked: createAction.trigger()
            }
            Item { Layout.fillWidth: true }
            Repeater {
                model: page.roomy ? [favoriteAction, pinAction, stickyAction] : []
                delegate: QQC2.ToolButton {
                    required property var modelData
                    visible: modelData.visible
                    icon.name: modelData.icon.name
                    text: modelData.text
                    display: QQC2.AbstractButton.IconOnly
                    checkable: modelData.checkable
                    checked: modelData.checked
                    QQC2.ToolTip.text: text
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    onClicked: modelData.trigger()
                }
            }
            // Everything else about the note: its file, archive and the way out of it.
            QQC2.ToolButton {
                id: noteActionsButton
                visible: !page.isDraft
                text: i18n("Note Actions")
                icon.name: "overflow-menu"
                display: QQC2.AbstractButton.IconOnly
                QQC2.ToolTip.text: text
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                // One plain list, no dividers: the icons first when there was no room for them in the bar.
                onClicked: noteSheet.showActions(i18n("Note"), (page.roomy ? [] : [favoriteAction, pinAction, stickyAction])
                    .concat([archiveAction, showFileAction, trashAction, restoreAction, purgeAction]), noteActionsButton)
            }
        }
    }

    // A menu in a wide window, a bottom sheet in a narrow one.
    ActionDrawer { id: noteSheet }

    Kirigami.PromptDialog {
        id: purgeDialog
        title: i18n("Delete Permanently")
        subtitle: i18n("This note will be deleted for good.")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: { page.backend.purgeNote(page.noteId); page.close() }
    }

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        QQC2.TextField {
            id: titleField
            Layout.fillWidth: true
            placeholderText: i18n("Title")
            font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.3
            readOnly: page.trashed
            Component.onCompleted: { text = page.note.title; if (page.isDraft) forceActiveFocus() }
            onTextEdited: saveTimer.restart()
            onAccepted: bodyArea.forceActiveFocus()
        }

        QQC2.TextArea {
            id: bodyArea
            Layout.fillWidth: true
            Layout.minimumHeight: Kirigami.Units.gridUnit * 12
            visible: !previewAction.checked
            placeholderText: i18n("Write in Markdown…")
            wrapMode: TextEdit.Wrap
            readOnly: page.trashed
            Component.onCompleted: text = page.note.body
            onTextChanged: if (activeFocus) saveTimer.restart()
        }
        QQC2.Label {
            Layout.fillWidth: true
            visible: previewAction.checked
            text: bodyArea.text
            textFormat: Text.MarkdownText
            wrapMode: Text.Wrap
            onLinkActivated: link => Qt.openUrlExternally(link)
        }

        Kirigami.Separator { Layout.fillWidth: true }

        Kirigami.FormLayout {
            Layout.fillWidth: true

            QQC2.ComboBox {
                Kirigami.FormData.label: i18n("Folder:")
                enabled: !page.trashed
                textRole: "label"
                valueRole: "id"
                model: page.folderList.map(f => ({ id: f.id, label: " ".repeat(f.depth) + f.title }))
                currentIndex: Math.max(0, page.folderList.findIndex(f => f.id === page.note.folderId))
                onActivated: page.setFolder(currentValue)
            }

            RowLayout {
                Kirigami.FormData.label: i18n("Rating:")
                visible: page.note.rated === true
                spacing: 0
                Repeater {
                    model: 5
                    delegate: QQC2.ToolButton {
                        required property int index
                        icon.name: index < page.note.rating ? "starred-symbolic" : "non-starred-symbolic"
                        text: i18np("%1 star", "%1 stars", index + 1)
                        display: QQC2.AbstractButton.IconOnly
                        // Pressing the star that is the rating clears it.
                        onClicked: page.setRating(index + 1 === page.note.rating ? 0 : index + 1)
                    }
                }
            }
            QQC2.Label {
                Kirigami.FormData.label: i18n("Rating:")
                visible: page.note.rated !== true
                text: i18n("This folder does not rate its notes.")
                color: Kirigami.Theme.disabledTextColor
            }

            ColorPicker {
                Kirigami.FormData.label: i18n("Color:")
                Layout.fillWidth: true
                presets: page.presets
                color: page.note.color ? page.note.color : ""
                onPicked: hex => page.setColor(hex)
            }

            Flow {
                Kirigami.FormData.label: i18n("Tags:")
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing
                Repeater {
                    model: page.note.tags
                    delegate: Kirigami.Chip {
                        required property var modelData
                        text: typeof modelData === "string" ? modelData : modelData.name
                        checkable: false
                        closable: !page.trashed
                        onRemoved: page.setTags(page.note.tags.map(t => typeof t === "string" ? t : t.name).filter(n => n !== text))
                    }
                }
                QQC2.TextField {
                    visible: !page.trashed
                    placeholderText: i18n("Add tag…")
                    onAccepted: {
                        if (text.trim() === "") return
                        const names = page.note.tags.map(t => typeof t === "string" ? t : t.name)
                        page.setTags(names.concat([text.trim()]))
                        text = ""
                    }
                }
            }
        }
    }
}
