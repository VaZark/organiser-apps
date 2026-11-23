import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.notes

/// A note as a list row: information only (colour, title, excerpt, folder, rating, pinned, favorite).
/// It has no buttons: a click opens the note, whose page has Favorite, Pin, Sticky Note, Archive and
/// Trash (and Restore and Delete Permanently for a trashed note).
Kirigami.SwipeListItem {
    id: row
    required property var note
    required property var backend
    signal opened()
    /// A hairline above the row: only between two rows, so the last one has none hanging below.
    property bool divider: false

    onClicked: opened()
    // The note's colour is the stripe at the row's left edge, as in Todo's list; the row itself is
    // plain, and a hairline between two rows keeps them apart.
    background: Item {
        Rectangle {
            anchors.fill: parent
            anchors.topMargin: 1
            anchors.bottomMargin: 1
            color: row.containsMouse ? Qt.alpha(Kirigami.Theme.highlightColor, 0.15) : "transparent"
            Rectangle {
                visible: row.note.effectiveColor !== ""
                width: Kirigami.Units.smallSpacing
                height: parent.height
                color: row.note.effectiveColor !== "" ? row.note.effectiveColor : "transparent"
            }
        }
        Kirigami.Separator { visible: row.divider; anchors { left: parent.left; right: parent.right; top: parent.top } }
    }
    leftPadding: Kirigami.Units.largeSpacing + Kirigami.Units.smallSpacing
    contentItem: RowLayout {
        spacing: Kirigami.Units.largeSpacing
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
        TagChip {
            visible: row.note.chip
            text: row.note.folderPath ? row.note.folderPath : ""
            tint: row.note.folderColor ? row.note.folderColor : Kirigami.Theme.highlightColor
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
}
