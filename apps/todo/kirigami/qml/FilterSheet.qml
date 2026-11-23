import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.todo

/// Which tasks every view shows: search, status, priority, pinned, overdue, epic, tags, and
/// whether finished tasks and subtasks appear. Like Notes' filter sheet, with more to choose from.
Kirigami.OverlaySheet {
    id: sheet
    readonly property QtObject placement: SheetPlacement { popup: sheet; contentHeight: content.implicitHeight }
    required property var backend
    /// Changing the epic is the window's to do (it also repaints the sidebar).
    signal epicPicked(int epic, bool noEpic)

    readonly property var st: JSON.parse(backend.filters)
    readonly property var tagList: JSON.parse(backend.tags)
    readonly property var statusList: JSON.parse(backend.statuses)
    readonly property var epicList: JSON.parse(backend.epics)
    readonly property var picked: backend.tagFilter === "" ? [] : backend.tagFilter.split(",")
    readonly property var priorities: ["highest", "high", "medium", "low", "lowest"]
    readonly property var epicChoices: [{ id: -1, title: i18n("Any epic") }, { id: -2, title: i18n("Unassigned") }].concat(epicList)

    title: i18n("Filter Tasks")
    onOpened: search.forceActiveFocus()

    function setPicked(list) { backend.tagFilter = list.join(","); backend.applyFilters() }
    function toggleTag(name) { setPicked(picked.indexOf(name) >= 0 ? picked.filter(t => t !== name) : picked.concat([name])) }

    ColumnLayout {
        id: content
        spacing: Kirigami.Units.largeSpacing
        Kirigami.FormLayout {
            Layout.fillWidth: true
            Kirigami.SearchField {
                id: search
                Kirigami.FormData.label: i18n("Search:")
                Layout.fillWidth: true
                text: sheet.backend.search
                onTextChanged: if (text !== sheet.backend.search) { sheet.backend.search = text; sheet.backend.applyFilters() }
            }
            QQC2.ComboBox {
                Kirigami.FormData.label: i18n("Epic:")
                model: sheet.epicChoices
                textRole: "title"
                currentIndex: {
                    if (sheet.backend.noEpic) return 1
                    const i = sheet.epicChoices.findIndex(e => e.id === sheet.backend.epic)
                    return i < 0 ? 0 : i
                }
                onActivated: i => {
                    const id = sheet.epicChoices[i].id
                    sheet.epicPicked(id < 0 ? -1 : id, id === -2)
                }
            }
            Flow {
                Kirigami.FormData.label: i18n("Status:")
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing
                Repeater {
                    model: sheet.statusList
                    delegate: Kirigami.Chip {
                        required property var modelData
                        text: modelData.name
                        checkable: true
                        closable: false
                        checked: sheet.st.statuses.indexOf(modelData.id) >= 0
                        onToggled: sheet.backend.toggleFilter("status", String(modelData.id))
                    }
                }
            }
            Flow {
                Kirigami.FormData.label: i18n("Priority:")
                Layout.fillWidth: true
                spacing: Kirigami.Units.smallSpacing
                Repeater {
                    model: sheet.priorities
                    delegate: Kirigami.Chip {
                        required property string modelData
                        text: modelData.charAt(0).toUpperCase() + modelData.slice(1)
                        checkable: true
                        closable: false
                        checked: sheet.st.priorities.indexOf(modelData) >= 0
                        onToggled: sheet.backend.toggleFilter("priority", modelData)
                    }
                }
            }
            QQC2.CheckBox {
                Kirigami.FormData.label: i18n("Show only:")
                text: i18n("Pinned tasks")
                checked: sheet.st.pinned === true
                onToggled: sheet.backend.toggleFilter("pinned", checked ? "1" : "0")
            }
            QQC2.CheckBox {
                text: i18n("Overdue tasks")
                checked: sheet.st.overdue === true
                onToggled: sheet.backend.toggleFilter("overdue", checked ? "1" : "0")
            }
            QQC2.CheckBox {
                Kirigami.FormData.label: i18n("Include:")
                text: i18n("Finished tasks")
                checked: sheet.backend.showClosed
                onToggled: { sheet.backend.showClosed = checked; sheet.backend.applyFilters() }
            }
            QQC2.CheckBox {
                text: i18n("Subtasks")
                checked: sheet.backend.showSubtasks
                onToggled: { sheet.backend.showSubtasks = checked; sheet.backend.applyFilters() }
            }
            TagPicker {
                Kirigami.FormData.label: i18n("Tags:")
                Layout.fillWidth: true
                allTags: sheet.tagList
                picked: sheet.picked
                onToggled: name => sheet.toggleTag(name)
            }
            QQC2.Label {
                Layout.maximumWidth: Kirigami.Units.gridUnit * 20
                wrapMode: Text.Wrap
                color: Kirigami.Theme.disabledTextColor
                text: i18n("A task needs every tag you select, and any of the statuses or priorities.")
            }
        }
    }

    footer: QQC2.DialogButtonBox {
        QQC2.Button {
            text: i18n("Clear Filters")
            icon.name: "edit-clear-all"
            enabled: sheet.st.count > 0
            QQC2.DialogButtonBox.buttonRole: QQC2.DialogButtonBox.ResetRole
            onClicked: sheet.backend.clearFilters()
        }
        QQC2.Button {
            text: i18n("Close")
            icon.name: "dialog-close"
            QQC2.DialogButtonBox.buttonRole: QQC2.DialogButtonBox.RejectRole
            onClicked: sheet.close()
        }
    }
}
