import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.todo

/// The filters that every view shares: search and tags (the sidebar sets scope and epic). It sits
/// under the toolbar of the list, the board and the matrix.
QQC2.ToolBar {
    id: bar
    required property var backend
    readonly property var allTags: JSON.parse(backend.tags)
    readonly property var picked: backend.tagFilter === "" ? [] : backend.tagFilter.split(",")
    function setPicked(list) { backend.tagFilter = list.join(","); backend.applyFilters() }
    function toggleTag(name) { setPicked(picked.indexOf(name) >= 0 ? picked.filter(t => t !== name) : picked.concat([name])) }

    contentItem: ColumnLayout {
        spacing: Kirigami.Units.smallSpacing
        Kirigami.SearchField {
            Layout.fillWidth: true
            text: bar.backend.search
            onAccepted: { bar.backend.search = text; bar.backend.applyFilters() }
        }
        Flow {
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing
            visible: bar.allTags.length > 0
            Repeater {
                model: bar.allTags
                delegate: QQC2.AbstractButton {
                    required property var modelData
                    readonly property bool on: bar.picked.indexOf(modelData.name) >= 0
                    onClicked: bar.toggleTag(modelData.name)
                    contentItem: TagChip {
                        text: modelData.name
                        tint: modelData.color ? modelData.color : Kirigami.Theme.highlightColor
                        opacity: parent.on ? 1 : 0.55
                    }
                    background: null
                    padding: 0
                }
            }
        }
    }
}
