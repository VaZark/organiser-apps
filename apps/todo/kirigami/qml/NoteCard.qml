import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.todo

/// A task in the Notes view: a card whose width the row gives it, notes drawn under the title.
/// View only; a click opens the task.
Item {
    id: note
    required property var task
    signal clicked()
    implicitHeight: card.implicitHeight

    Kirigami.AbstractCard {
        id: card
        width: note.width
        opacity: note.task.closed ? 0.6 : 1
        onClicked: note.clicked()
        contentItem: ColumnLayout {
            spacing: Kirigami.Units.smallSpacing
            RowLayout {
                StatusDot { tint: note.task.statusColor ? note.task.statusColor : Kirigami.Theme.disabledTextColor }
                QQC2.Label {
                    Layout.fillWidth: true
                    text: note.task.title
                    font.bold: true
                    wrapMode: Text.Wrap
                    font.strikeout: note.task.closed
                }
                Kirigami.Icon { visible: note.task.pinned; source: "pin"; Layout.preferredWidth: Kirigami.Units.iconSizes.small; Layout.preferredHeight: width }
            }
            QQC2.Label {
                visible: text !== ""
                Layout.fillWidth: true
                font: Kirigami.Theme.smallFont
                color: Kirigami.Theme.disabledTextColor
                text: [note.task.epicTitle, note.task.due ? i18n("due %1", note.task.due) : ""].filter(s => s).join(" · ")
            }
            QQC2.Label {
                visible: note.task.notes !== ""
                Layout.fillWidth: true
                text: note.task.notes
                textFormat: Text.MarkdownText
                wrapMode: Text.Wrap
            }
            Flow {
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing
                Repeater { model: note.task.tags; TagChip { required property string modelData; text: modelData } }
            }
        }
    }
}
