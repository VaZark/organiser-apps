import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.recipes

/// What was cooked and what is planned, month by month (weeks start on Monday).
Kirigami.ScrollablePage {
    id: page
    required property var backend
    readonly property var cal: JSON.parse(backend.calendar)
    readonly property var days: cal.weeks ? cal.weeks.reduce((all, w) => all.concat(w), []) : []
    readonly property var meals: days.filter(d => d.inMonth).reduce((all, d) => all.concat(d.meals), [])
    readonly property var weekdays: [i18n("Mon"), i18n("Tue"), i18n("Wed"), i18n("Thu"), i18n("Fri"), i18n("Sat"), i18n("Sun")]
    title: cal.title ? cal.title : i18n("Log")

    actions: [
        Kirigami.Action {
            text: i18n("Log Meal…")
            icon.name: "list-add"
            shortcut: StandardKey.New
            onTriggered: applicationWindow().logMeal("", -1)
        },
        Kirigami.Action {
            text: i18n("Previous Month")
            icon.name: "go-previous"
            onTriggered: page.backend.shiftMonth(-1)
        },
        Kirigami.Action {
            text: i18n("Today")
            icon.name: "go-jump-today"
            onTriggered: page.backend.thisMonth()
        },
        Kirigami.Action {
            text: i18n("Next Month")
            icon.name: "go-next"
            onTriggered: page.backend.shiftMonth(1)
        }
    ]

    ColumnLayout {
        spacing: Kirigami.Units.largeSpacing

        GridLayout {
            Layout.fillWidth: true
            Layout.margins: Kirigami.Units.largeSpacing
            columns: 7
            rowSpacing: 1
            columnSpacing: 1
            Repeater {
                model: page.weekdays
                delegate: QQC2.Label {
                    required property string modelData
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    horizontalAlignment: Text.AlignHCenter
                    text: modelData
                    color: Kirigami.Theme.disabledTextColor
                }
            }
            Repeater {
                model: page.days
                delegate: QQC2.AbstractButton {
                    id: cell
                    required property var modelData
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    implicitHeight: Kirigami.Units.gridUnit * 4
                    opacity: modelData.inMonth ? 1 : 0.5
                    onClicked: applicationWindow().logMeal(modelData.date, -1)
                    background: Rectangle {
                        color: cell.modelData.today ? Qt.alpha(Kirigami.Theme.highlightColor, 0.2) : cell.hovered ? Kirigami.Theme.hoverColor : Kirigami.Theme.backgroundColor
                        border.color: cell.modelData.today ? Kirigami.Theme.highlightColor : Kirigami.Theme.disabledTextColor
                        border.width: 1
                    }
                    contentItem: ColumnLayout {
                        spacing: 0
                        QQC2.Label { text: cell.modelData.day; font.bold: cell.modelData.today }
                        Repeater {
                            model: cell.modelData.meals.slice(0, 2)
                            delegate: QQC2.Label {
                                required property var modelData
                                Layout.fillWidth: true
                                text: modelData.title
                                elide: Text.ElideRight
                                font.pointSize: Kirigami.Theme.smallFont.pointSize
                                font.italic: modelData.planned
                            }
                        }
                        QQC2.Label {
                            visible: cell.modelData.meals.length > 2
                            text: i18n("+%1 more", cell.modelData.meals.length - 2)
                            font.pointSize: Kirigami.Theme.smallFont.pointSize
                            color: Kirigami.Theme.disabledTextColor
                        }
                        Item { Layout.fillHeight: true }
                    }
                }
            }
        }

        Kirigami.Heading {
            Layout.leftMargin: Kirigami.Units.largeSpacing
            level: 3
            text: i18n("This Month")
            visible: page.meals.length > 0
        }
        Repeater {
            model: page.meals
            delegate: QQC2.ItemDelegate {
                id: row
                required property var modelData
                Layout.fillWidth: true
                onClicked: if (modelData.recipe) applicationWindow().openRecipe(modelData.recipe)
                contentItem: RowLayout {
                    QQC2.Label { Layout.fillWidth: true; text: row.modelData.title; elide: Text.ElideRight }
                    QQC2.Label {
                        text: row.modelData.date + (row.modelData.timeLabel !== "" ? ", " + row.modelData.timeLabel : "") + " · " + (row.modelData.planned ? i18n("planned") : i18n("cooked"))
                        color: Kirigami.Theme.disabledTextColor
                    }
                    QQC2.ToolButton {
                        icon.name: "edit-delete"
                        text: i18n("Delete Meal")
                        display: QQC2.AbstractButton.IconOnly
                        onClicked: page.backend.deleteMeal(row.modelData.id)
                        QQC2.ToolTip.text: text
                        QQC2.ToolTip.visible: hovered
                    }
                }
            }
        }
    }
}
