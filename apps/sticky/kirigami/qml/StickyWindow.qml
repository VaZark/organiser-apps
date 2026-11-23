import QtQuick
import QtQuick.Layouts
import QtQuick.Window
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

/// One sticky: a frameless coloured note with a small toolbar, a title and text. The text is
/// drawn as Markdown, or edited as its source. The note's data is the backend's; this only
/// presents it and forwards edits.
Window {
    id: win

    required property string key
    required property var backend
    /// "All notes" was chosen.
    signal showAll()
    /// The sticky was taken off the desktop.
    signal hidden()

    // Re-read when the backend says so (a colour, a change made elsewhere); not on every key typed.
    readonly property var note: { backend.revision; return JSON.parse(backend.sticky(key)) }
    readonly property color paper: note.paper ? note.paper : "#fff59d"
    readonly property color ink: note.ink ? note.ink : "#2b2b2b"
    property bool editing: false
    readonly property Item sheetItem: sheet

    width: Kirigami.Units.gridUnit * 17
    height: Kirigami.Units.gridUnit * 18
    minimumWidth: Kirigami.Units.gridUnit * 11
    minimumHeight: Kirigami.Units.gridUnit * 9
    visible: true
    title: note.title ? note.title : i18n("Sticky")
    flags: Qt.Window | Qt.FramelessWindowHint
    color: "transparent"

    function hide() {
        backend.setDesktop(key, false)
        win.hidden()
    }

    Shortcut {
        sequences: [StandardKey.Close]
        onActivated: win.hide()
    }

    Rectangle {
        id: sheet
        anchors.fill: parent
        radius: Kirigami.Units.cornerRadius
        color: win.paper
        border.width: 1
        border.color: Qt.tint(win.paper, Qt.alpha(win.ink, 0.25))

        // The note's colours, for everything inside it.
        Kirigami.Theme.inherit: false
        Kirigami.Theme.colorSet: Kirigami.Theme.View
        Kirigami.Theme.backgroundColor: win.paper
        Kirigami.Theme.textColor: win.ink
        Kirigami.Theme.disabledTextColor: Qt.alpha(win.ink, 0.55)
        Kirigami.Theme.highlightColor: win.ink

        ColumnLayout {
            anchors.fill: parent
            spacing: 0

            // The strip to move the note by, with its tools.
            RowLayout {
                Layout.fillWidth: true
                Layout.margins: Kirigami.Units.smallSpacing
                spacing: 0

                QQC2.ToolButton {
                    icon.name: "list-add"
                    icon.color: win.ink
                    display: QQC2.AbstractButton.IconOnly
                    text: i18n("New Note in…")
                    QQC2.ToolTip.text: text
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    onClicked: newMenu.popup(this, 0, height)
                }
                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    implicitHeight: Kirigami.Units.gridUnit * 2
                    DragHandler {
                        target: null
                        onActiveChanged: if (active) win.startSystemMove()
                    }
                }
                QQC2.ToolButton {
                    icon.name: win.editing ? "document-preview" : "document-edit"
                    icon.color: win.ink
                    display: QQC2.AbstractButton.IconOnly
                    text: win.editing ? i18n("Read") : i18n("Edit Markdown")
                    QQC2.ToolTip.text: text
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    onClicked: win.editing = !win.editing
                }
                QQC2.ToolButton {
                    icon.name: "overflow-menu"
                    icon.color: win.ink
                    display: QQC2.AbstractButton.IconOnly
                    text: i18n("More")
                    QQC2.ToolTip.text: text
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    onClicked: moreMenu.popup(this, 0, height)
                }
                QQC2.ToolButton {
                    icon.name: "window-close"
                    icon.color: win.ink
                    display: QQC2.AbstractButton.IconOnly
                    text: i18n("Hide from the Desktop")
                    QQC2.ToolTip.text: i18n("Hide from the desktop (it stays in Sticky Notes)")
                    QQC2.ToolTip.visible: hovered
                    QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                    onClicked: win.hide()
                }
            }

            QQC2.TextField {
                id: titleField
                Layout.fillWidth: true
                Layout.leftMargin: Kirigami.Units.smallSpacing
                Layout.rightMargin: Kirigami.Units.smallSpacing
                placeholderText: i18n("Title")
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.15
                font.bold: true
                color: win.ink
                placeholderTextColor: Qt.alpha(win.ink, 0.55)
                background: null
                text: win.note.title ? win.note.title : ""
                onTextEdited: win.backend.setTitle(win.key, text)
            }

            QQC2.ScrollView {
                visible: !win.editing
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.leftMargin: Kirigami.Units.smallSpacing
                Layout.rightMargin: Kirigami.Units.smallSpacing

                // Reading: the Markdown drawn.
                TextEdit {
                    width: parent.width
                    readOnly: true
                    selectByMouse: true
                    wrapMode: TextEdit.Wrap
                    textFormat: TextEdit.MarkdownText
                    color: win.ink
                    selectionColor: Qt.alpha(win.ink, 0.3)
                    selectedTextColor: win.ink
                    font: Kirigami.Theme.defaultFont
                    padding: Kirigami.Units.smallSpacing
                    // What is typed in the editor, so the reading view is never behind it.
                    text: bodyField.text
                    onLinkActivated: link => Qt.openUrlExternally(link)
                }
            }

            // Editing: the source.
            QQC2.ScrollView {
                visible: win.editing
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.leftMargin: Kirigami.Units.smallSpacing
                Layout.rightMargin: Kirigami.Units.smallSpacing

                QQC2.TextArea {
                    id: bodyField
                    placeholderText: i18n("Write in Markdown…")
                    color: win.ink
                    placeholderTextColor: Qt.alpha(win.ink, 0.55)
                    selectionColor: Qt.alpha(win.ink, 0.3)
                    selectedTextColor: win.ink
                    wrapMode: TextEdit.Wrap
                    background: null
                    text: win.note.body ? win.note.body : ""
                    onTextChanged: if (activeFocus) win.backend.setBody(win.key, text)
                }
            }

            // The app the note came from; the corner beside it resizes.
            RowLayout {
                Layout.fillWidth: true
                Layout.margins: Kirigami.Units.smallSpacing
                QQC2.Label {
                    text: win.note.appName ? win.note.appName : ""
                    color: Qt.alpha(win.ink, 0.7)
                    font: Kirigami.Theme.smallFont
                    Layout.leftMargin: Kirigami.Units.smallSpacing
                }
                Item { Layout.fillWidth: true }
                Kirigami.Icon {
                    source: "transform-scale"
                    color: Qt.alpha(win.ink, 0.5)
                    isMask: true
                    implicitWidth: Kirigami.Units.iconSizes.small
                    implicitHeight: implicitWidth
                    HoverHandler { cursorShape: Qt.SizeFDiagCursor }
                    DragHandler {
                        target: null
                        onActiveChanged: if (active) win.startSystemResize(Qt.RightEdge | Qt.BottomEdge)
                    }
                }
            }
        }
    }

    // "+": which linked app to write a note in.
    QQC2.Menu {
        id: newMenu
        QQC2.MenuItem {
            enabled: false
            visible: appItems.count === 0
            text: i18n("Link an app in Sticky Notes first.")
        }
        Instantiator {
            id: appItems
            // An integer model: the entries are read by index.
            readonly property var apps: JSON.parse(win.backend.links).filter(l => l.enabled)
            model: apps.length
            delegate: QQC2.MenuItem {
                required property int index
                readonly property var app: appItems.apps[index]
                text: app.title
                onTriggered: win.backend.newIn(app.appId)
            }
            onObjectAdded: (i, o) => newMenu.insertItem(i + 1, o)
            onObjectRemoved: (i, o) => newMenu.removeItem(o)
        }
    }

    QQC2.Menu {
        id: moreMenu
        QQC2.MenuItem {
            text: i18n("All Notes")
            icon.name: "view-list-details"
            onTriggered: win.showAll()
        }
        QQC2.Menu {
            id: colourMenu
            title: i18n("Colour")
            QQC2.MenuItem {
                text: i18n("Default")
                checkable: true
                checked: !win.note.colored
                onTriggered: win.backend.setColor(win.key, "")
            }
            QQC2.MenuSeparator {}
            Instantiator {
                id: swatchItems
                readonly property var swatches: JSON.parse(win.backend.palette).slice(0, 5)
                model: swatches.length
                delegate: QQC2.MenuItem {
                    required property int index
                    readonly property var swatch: swatchItems.swatches[index]
                    text: swatch.name
                    icon.name: "format-fill-color"
                    icon.color: swatch.hex
                    checkable: true
                    checked: win.note.colored && win.note.paper === swatch.hex
                    onTriggered: win.backend.setColor(win.key, swatch.hex)
                }
                onObjectAdded: (i, o) => colourMenu.insertItem(i + 2, o)
                onObjectRemoved: (i, o) => colourMenu.removeItem(o)
            }
        }
    }
}
