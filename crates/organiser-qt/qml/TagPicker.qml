import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

/// Pick tags by typing: the chosen ones are chips you can remove, and a search field suggests
/// matching tags only once something is typed, so a long tag list is never laid out.
ColumnLayout {
    id: picker
    /// Every tag there is: `[{name, color}]`.
    property var allTags: []
    /// The names chosen.
    property var picked: []
    /// A tag was chosen or removed (the name either way).
    signal toggled(string name)

    readonly property int maxSuggestions: 8
    readonly property var matches: {
        const q = field.text.trim().toLowerCase()
        if (q === "") return []
        const out = []
        for (const t of allTags) {
            if (picked.indexOf(t.name) >= 0) continue
            const n = t.name.toLowerCase()
            // Names that start with the text come first.
            if (n.startsWith(q)) out.unshift(t)
            else if (n.indexOf(q) >= 0) out.push(t)
        }
        return out.slice(0, maxSuggestions)
    }

    function choose(name) {
        picker.toggled(name)
        field.text = ""
    }

    spacing: Kirigami.Units.smallSpacing

    Flow {
        Layout.fillWidth: true
        spacing: Kirigami.Units.smallSpacing
        visible: picker.picked.length > 0
        Repeater {
            model: picker.picked
            delegate: Kirigami.Chip {
                required property string modelData
                text: modelData
                closable: true
                onRemoved: picker.toggled(modelData)
            }
        }
    }

    Kirigami.SearchField {
        id: field
        Layout.fillWidth: true
        placeholderText: picker.allTags.length === 0 ? i18n("No tags yet") : i18n("Find a tag…")
        enabled: picker.allTags.length > 0
        onAccepted: if (picker.matches.length > 0) picker.choose(picker.matches[0].name)
        Keys.onEscapePressed: event => { if (text !== "") { text = ""; event.accepted = true } else event.accepted = false }
        Keys.onDownPressed: if (suggestions.opened) list.forceActiveFocus()

        QQC2.Popup {
            id: suggestions
            y: field.height
            width: field.width
            padding: 0
            focus: false
            closePolicy: QQC2.Popup.CloseOnEscape | QQC2.Popup.CloseOnPressOutsideParent
            visible: field.activeFocus && picker.matches.length > 0 || list.activeFocus
            contentItem: ListView {
                id: list
                implicitHeight: contentHeight
                model: picker.matches
                clip: true
                currentIndex: 0
                delegate: QQC2.ItemDelegate {
                    required property var modelData
                    required property int index
                    width: ListView.view.width
                    text: modelData.name
                    highlighted: ListView.isCurrentItem
                    onClicked: picker.choose(modelData.name)
                    Keys.onReturnPressed: picker.choose(modelData.name)
                }
            }
        }
    }
}
