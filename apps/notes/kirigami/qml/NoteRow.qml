import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

/// A note as a list row, with row actions.
Kirigami.SwipeListItem {
    id: row
    required property var note
    required property var backend
    signal opened()

    onClicked: opened()
    // The note colour tints the row.
    background: Rectangle {
        color: row.note.color !== ""
            ? Qt.alpha(row.note.color, row.containsMouse ? 0.35 : 0.22)
            : (row.containsMouse ? Qt.alpha(Kirigami.Theme.highlightColor, 0.15) : "transparent")
    }
    contentItem: RowLayout {
        spacing: Kirigami.Units.largeSpacing
        Rectangle {
            visible: row.note.color !== ""
            color: row.note.color
            radius: width / 2
            implicitWidth: Kirigami.Units.gridUnit * 0.7
            implicitHeight: implicitWidth
        }
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 0
            QQC2.Label {
                Layout.fillWidth: true
                text: row.note.title
                elide: Text.ElideRight
                font.bold: row.note.pinned
            }
            QQC2.Label {
                Layout.fillWidth: true
                visible: text !== ""
                text: row.note.excerpt.split("\n")[0]
                elide: Text.ElideRight
                font: Kirigami.Theme.smallFont
                color: Kirigami.Theme.disabledTextColor
            }
        }
        Kirigami.Chip {
            visible: row.note.chip
            text: row.note.folderTitle ? row.note.folderTitle : ""
            icon.name: row.note.folderIcon ? row.note.folderIcon : "folder"
            checkable: false
            closable: false
            down: false
        }
        QQC2.Label {
            visible: row.note.rated && row.note.rating > 0
            text: "★".repeat(row.note.rating)
        }
        Kirigami.Icon {
            visible: row.note.pinned
            source: "pin"
            Layout.preferredWidth: Kirigami.Units.iconSizes.small
            Layout.preferredHeight: Kirigami.Units.iconSizes.small
        }
        Kirigami.Icon {
            visible: row.note.favorite
            source: "emblem-favorite"
            Layout.preferredWidth: Kirigami.Units.iconSizes.small
            Layout.preferredHeight: Kirigami.Units.iconSizes.small
        }
    }
    actions: [
        Kirigami.Action {
            text: i18n("Restore")
            icon.name: "edit-undo"
            visible: row.note.trashed
            onTriggered: row.backend.restoreNote(row.note.id)
        },
        Kirigami.Action {
            text: i18n("Delete Permanently")
            icon.name: "edit-delete"
            visible: row.note.trashed
            onTriggered: row.backend.purgeNote(row.note.id)
        },
        Kirigami.Action {
            text: row.note.favorite ? i18n("Remove from Favorites") : i18n("Add to Favorites")
            icon.name: row.note.favorite ? "emblem-favorite" : "bookmark-new"
            visible: !row.note.trashed
            onTriggered: row.backend.setFavorite(row.note.id, !row.note.favorite)
        },
        Kirigami.Action {
            text: row.note.pinned ? i18n("Unpin") : i18n("Pin")
            icon.name: "pin"
            visible: !row.note.trashed
            onTriggered: row.backend.setPinned(row.note.id, !row.note.pinned)
        },
        Kirigami.Action {
            text: i18n("Move to Trash")
            icon.name: "user-trash"
            visible: !row.note.trashed
            onTriggered: row.backend.trashNote(row.note.id)
        }
    ]
}
