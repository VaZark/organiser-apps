import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.notes

/// A note as a card. `note` is one object of the page JSON; the card's colour is the note's own
/// (user data), tinted so text stays readable in any colour scheme.
Kirigami.AbstractCard {
    id: card
    required property var note
    signal opened()

    showClickFeedback: true
    onClicked: opened()
    Accessible.name: note.title

    background: Rectangle {
        radius: Kirigami.Units.cornerRadius
        color: card.note.effectiveColor !== "" ? Qt.alpha(card.note.effectiveColor, 0.32) : Kirigami.Theme.backgroundColor
        border.width: 1
        border.color: card.note.effectiveColor !== "" ? Qt.alpha(card.note.effectiveColor, 0.7) : Kirigami.ColorUtils.linearInterpolation(Kirigami.Theme.backgroundColor, Kirigami.Theme.textColor, 0.25)
    }

    contentItem: ColumnLayout {
        spacing: Kirigami.Units.smallSpacing
        RowLayout {
            Layout.fillWidth: true
            Kirigami.Heading {
                Layout.fillWidth: true
                level: 3
                text: card.note.title
                elide: Text.ElideRight
                wrapMode: Text.NoWrap
            }
            Kirigami.Icon {
                visible: card.note.pinned
                source: "pin"
                Layout.preferredWidth: Kirigami.Units.iconSizes.small
                Layout.preferredHeight: Kirigami.Units.iconSizes.small
            }
            Kirigami.Icon {
                visible: card.note.favorite
                source: "emblem-favorite"
                Layout.preferredWidth: Kirigami.Units.iconSizes.small
                Layout.preferredHeight: Kirigami.Units.iconSizes.small
            }
        }
        QQC2.Label {
            Layout.fillWidth: true
            visible: card.note.excerpt !== ""
            text: card.note.excerpt
            textFormat: Text.MarkdownText
            wrapMode: Text.Wrap
            maximumLineCount: 8
            elide: Text.ElideRight
        }
        QQC2.Label {
            visible: card.note.rated && card.note.rating > 0
            text: "★".repeat(card.note.rating) + "☆".repeat(5 - card.note.rating)
        }
        Flow {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing
            visible: card.note.tags.length > 0 || card.note.chip
            TagChip {
                visible: card.note.chip
                text: card.note.folderPath ? card.note.folderPath : ""
                tint: card.note.folderColor ? card.note.folderColor : Kirigami.Theme.highlightColor
            }
            Repeater {
                model: card.note.tags
                delegate: Kirigami.Chip {
                    required property var modelData
                    text: modelData.name
                    checkable: false
                    closable: false
                    down: false
                }
            }
        }
    }
}
