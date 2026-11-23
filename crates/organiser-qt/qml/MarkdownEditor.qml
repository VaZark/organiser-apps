import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.kde.syntaxhighlighting

/// A Markdown field in the manner of KleverNotes: a formatting toolbar, syntax highlighting while
/// editing, and a rendered preview. `committed(text)` fires when editing ends or the preview opens.
ColumnLayout {
    id: ed
    property alias text: area.text
    property string placeholder: ""
    property int minimumHeight: Kirigami.Units.gridUnit * 10
    signal committed(string text)

    spacing: 0

    // Wrap the selection (or the cursor) in `before`/`after`; `line` prefixes the current line.
    function wrap(before, after) {
        const s = area.selectionStart, e = area.selectionEnd
        const inner = area.text.substring(s, e)
        area.remove(s, e)
        area.insert(s, before + inner + after)
        area.select(s + before.length, s + before.length + inner.length)
        area.forceActiveFocus()
    }
    function line(prefix) {
        const at = area.text.lastIndexOf("\n", area.selectionStart - 1) + 1
        area.insert(at, prefix)
        area.forceActiveFocus()
    }
    function commit() { ed.committed(area.text) }

    QQC2.ToolBar {
        Layout.fillWidth: true
        contentItem: RowLayout {
            spacing: 0
            QQC2.ToolButton {
                id: previewToggle
                text: i18n("Preview")
                icon.name: "view-preview"
                checkable: true
                onToggled: if (checked) ed.commit()
            }
            Kirigami.Separator { Layout.fillHeight: true; Layout.margins: Kirigami.Units.smallSpacing }
            Repeater {
                model: [
                    { icon: "format-text-bold", tip: i18n("Bold"), run: () => ed.wrap("**", "**") },
                    { icon: "format-text-italic", tip: i18n("Italic"), run: () => ed.wrap("*", "*") },
                    { icon: "format-text-strikethrough", tip: i18n("Strikethrough"), run: () => ed.wrap("~~", "~~") },
                    { icon: "format-text-code", tip: i18n("Code"), run: () => ed.wrap("`", "`") },
                    { icon: "format-justify-fill", tip: i18n("Heading"), run: () => ed.line("## ") },
                    { icon: "format-list-unordered", tip: i18n("Bulleted list"), run: () => ed.line("- ") },
                    { icon: "checkbox", tip: i18n("Checklist"), run: () => ed.line("- [ ] ") },
                    { icon: "format-text-blockquote", tip: i18n("Quote"), run: () => ed.line("> ") },
                    { icon: "insert-link", tip: i18n("Link"), run: () => ed.wrap("[", "](https://)") }
                ]
                delegate: QQC2.ToolButton {
                    required property var modelData
                    visible: !previewToggle.checked
                    icon.name: modelData.icon
                    display: QQC2.AbstractButton.IconOnly
                    QQC2.ToolTip.text: modelData.tip
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    onClicked: modelData.run()
                }
            }
            Item { Layout.fillWidth: true }
        }
    }

    StackLayout {
        Layout.fillWidth: true
        Layout.preferredHeight: ed.minimumHeight
        currentIndex: previewToggle.checked ? 1 : 0
        QQC2.ScrollView {
            QQC2.TextArea {
                id: area
                placeholderText: ed.placeholder
                wrapMode: TextEdit.Wrap
                selectByMouse: true
                textFormat: TextEdit.PlainText
                font.family: "monospace"
                onEditingFinished: ed.commit()
                SyntaxHighlighter {
                    textEdit: area
                    definition: "Markdown"
                }
            }
        }
        QQC2.ScrollView {
            QQC2.TextArea {
                readOnly: true
                wrapMode: TextEdit.Wrap
                textFormat: TextEdit.MarkdownText
                text: area.text !== "" ? area.text : i18n("Nothing to preview.")
                background: null
                onLinkActivated: link => Qt.openUrlExternally(link)
            }
        }
    }
}
