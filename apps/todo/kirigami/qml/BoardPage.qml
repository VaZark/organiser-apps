import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.todo

/// One column per status; a card moves by the menu on it or by dragging it onto another column.
Kirigami.Page {
    id: page
    required property var backend
    signal openTask(int id)
    title: i18n("Board")
    padding: Kirigami.Units.smallSpacing

    readonly property var statuses: JSON.parse(backend.statuses)
    readonly property var tasks: JSON.parse(backend.allTasks)

    QQC2.ScrollView {
        anchors.fill: parent
        contentWidth: columns.implicitWidth
        RowLayout {
            id: columns
            height: page.height - Kirigami.Units.largeSpacing * 2
            spacing: Kirigami.Units.largeSpacing
            Repeater {
                model: page.statuses
                delegate: Rectangle {
                    id: col
                    required property var modelData
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 16
                    Layout.fillHeight: true
                    radius: Kirigami.Units.cornerRadius
                    color: Kirigami.Theme.alternateBackgroundColor
                    readonly property var cards: page.tasks.filter(t => t.statusId === modelData.id)

                    DropArea {
                        anchors.fill: parent
                        onDropped: drop => { if (drop.source && drop.source.taskId !== undefined) page.backend.setStatus(drop.source.taskId, col.modelData.id) }
                    }
                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.smallSpacing
                        RowLayout {
                            StatusDot { tint: col.modelData.color ? col.modelData.color : Kirigami.Theme.disabledTextColor }
                            Kirigami.Heading { level: 4; text: col.modelData.name + " (" + col.cards.length + ")"; Layout.fillWidth: true }
                        }
                        ListView {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            clip: true
                            spacing: Kirigami.Units.smallSpacing
                            model: col.cards
                            delegate: Kirigami.AbstractCard {
                                id: card
                                required property var modelData
                                property int taskId: modelData.id
                                width: ListView.view.width
                                Drag.active: dragHandler.active
                                Drag.source: card
                                Drag.hotSpot: Qt.point(width / 2, height / 2)
                                DragHandler {
                                    id: dragHandler
                                    onActiveChanged: if (!active) { card.Drag.drop(); card.x = 0; card.y = 0 }
                                }
                                onClicked: page.openTask(modelData.id)
                                background: Rectangle {
                                    radius: Kirigami.Units.cornerRadius
                                    color: card.modelData.color ? Qt.alpha(card.modelData.color, 0.32) : Kirigami.Theme.backgroundColor
                                    border.width: 1
                                    border.color: card.modelData.color ? Qt.alpha(card.modelData.color, 0.7) : Kirigami.ColorUtils.linearInterpolation(Kirigami.Theme.backgroundColor, Kirigami.Theme.textColor, 0.25)
                                }
                                contentItem: ColumnLayout {
                                    QQC2.Label { text: card.modelData.title; wrapMode: Text.Wrap; Layout.fillWidth: true; font.bold: card.modelData.pinned }
                                    QQC2.Label {
                                        visible: text !== ""
                                        font: Kirigami.Theme.smallFont
                                        color: Kirigami.Theme.disabledTextColor
                                        text: [card.modelData.epicTitle, applicationWindow().dates.show(card.modelData.due)].filter(s => s).join(" · ")
                                    }
                                    Flow {
                                        Layout.fillWidth: true
                                        spacing: Kirigami.Units.smallSpacing
                                        Repeater { model: card.modelData.tags; TagChip { required property string modelData; text: modelData } }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
