import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.organiser.todo

/// The tag vocabulary: a tag may sit under another (`parent`), and has a colour of its own.
Kirigami.ScrollablePage {
    id: page
    required property var backend
    title: i18n("Tags")
    readonly property var tags: JSON.parse(backend.tags)
    actions: Kirigami.Action { text: i18n("New tag"); icon.name: "list-add"; onTriggered: newTag.open() }

    Kirigami.Dialog {
        id: newTag
        title: i18n("New tag")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        ColumnLayout {
            QQC2.TextField { id: newTagName; placeholderText: i18n("Name"); Layout.fillWidth: true }
            QQC2.ComboBox {
                id: tagParent
                Layout.fillWidth: true
                model: [i18n("No parent")].concat(page.tags.map(t => t.name))
            }
        }
        onAccepted: if (newTagName.text.trim() !== "") {
            page.backend.createTag(newTagName.text.trim(), tagParent.currentIndex > 0 ? tagParent.currentText : "")
            newTagName.text = ""
            tagParent.currentIndex = 0
        }
    }
    Kirigami.Dialog {
        id: recolor
        property string target
        title: i18n("Colour of %1", target)
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        QQC2.TextField { id: tagColor; placeholderText: "#rrggbb"; Layout.fillWidth: true }
        onAccepted: page.backend.setTagColor(target, tagColor.text.trim())
    }

    ListView {
        id: list
        model: page.tags
        delegate: QQC2.ItemDelegate {
            id: d
            required property var modelData
            width: ListView.view.width
            leftPadding: Kirigami.Units.largeSpacing + (modelData.parent ? Kirigami.Units.gridUnit * 1.5 : 0)
            contentItem: RowLayout {
                StatusDot { tint: d.modelData.color ? d.modelData.color : Kirigami.Theme.disabledTextColor }
                QQC2.Label { Layout.fillWidth: true; text: d.modelData.name }
                QQC2.ToolButton { icon.name: "color-management"; onClicked: { recolor.target = d.modelData.name; tagColor.text = d.modelData.color ? d.modelData.color : ""; recolor.open() } }
                QQC2.ToolButton { icon.name: "edit-delete"; onClicked: page.backend.deleteTag(d.modelData.name, false) }
            }
        }
        Kirigami.PlaceholderMessage {
            anchors.centerIn: parent
            width: parent.width - Kirigami.Units.gridUnit * 4
            visible: list.count === 0
            icon.name: "tag"
            text: i18n("No tags yet")
            explanation: i18n("Add tags to a task from its details.")
        }
    }
}
