import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.notes

/// Which notes the page shows: pinned, favourites, ratings and tags.
Kirigami.OverlaySheet {
    id: sheet
    readonly property QtObject placement: SheetPlacement { popup: sheet; contentHeight: content.implicitHeight }
    required property var backend
    readonly property var st: JSON.parse(backend.filters)
    readonly property var tagList: JSON.parse(backend.tags)

    title: i18n("Filter Notes")

    ColumnLayout {
        id: content
        spacing: Kirigami.Units.largeSpacing
        Kirigami.FormLayout {
            Layout.fillWidth: true
            QQC2.CheckBox {
                Kirigami.FormData.label: i18n("Show only:")
                text: i18n("Pinned notes")
                checked: sheet.st.pinned === true
                onToggled: sheet.backend.toggleFilter("pinned", checked ? 1 : 0)
            }
            QQC2.CheckBox {
                text: i18n("Favorites")
                checked: sheet.st.favorite === true
                onToggled: sheet.backend.toggleFilter("favorite", checked ? 1 : 0)
            }
            Flow {
                Kirigami.FormData.label: i18n("Rating:")
                spacing: Kirigami.Units.smallSpacing
                Repeater {
                    model: [1, 2, 3, 4, 5]
                    delegate: QQC2.CheckBox {
                        required property int modelData
                        text: "★".repeat(modelData)
                        checked: sheet.st.ratings.indexOf(modelData) >= 0
                        onToggled: sheet.backend.toggleFilter("rating", modelData)
                    }
                }
            }
            TagPicker {
                Kirigami.FormData.label: i18n("Tags:")
                Layout.fillWidth: true
                allTags: sheet.tagList
                picked: sheet.st.tags
                onToggled: name => sheet.backend.toggleTagFilter(name)
            }
            QQC2.Label {
                Layout.maximumWidth: Kirigami.Units.gridUnit * 20
                wrapMode: Text.Wrap
                color: Kirigami.Theme.disabledTextColor
                text: i18n("A note needs every tag you select.")
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
