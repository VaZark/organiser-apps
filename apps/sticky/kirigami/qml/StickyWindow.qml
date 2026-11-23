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

    // The window is a little larger than the note: the margin holds its shadow.
    readonly property real shadowMargin: Kirigami.Units.gridUnit * 0.7
    width: Kirigami.Units.gridUnit * 17 + shadowMargin * 2
    height: Kirigami.Units.gridUnit * 18 + shadowMargin * 2
    minimumWidth: Kirigami.Units.gridUnit * 11 + shadowMargin * 2
    minimumHeight: Kirigami.Units.gridUnit * 9 + shadowMargin * 2
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

    Kirigami.ShadowedRectangle {
        id: sheet
        anchors.fill: parent
        anchors.margins: win.shadowMargin
        radius: Kirigami.Units.gridUnit * 0.6
        color: win.paper
        border.width: 1
        border.color: Qt.alpha(win.ink, 0.14)
        shadow.size: win.shadowMargin
        shadow.yOffset: Kirigami.Units.smallSpacing * 0.6
        shadow.color: Qt.rgba(0, 0, 0, 0.32)

        // The tools and the footer step back until the pointer is over the note or it is edited.
        readonly property real chrome: noteHover.hovered || win.editing || win.active ? 1 : 0.3
        HoverHandler { id: noteHover }

        // A soft light from the top, so the paper is not a flat slab.
        Rectangle {
            anchors.fill: parent
            radius: parent.radius
            gradient: Gradient {
                GradientStop { position: 0.0; color: Qt.rgba(1, 1, 1, 0.28) }
                GradientStop { position: 0.45; color: Qt.rgba(1, 1, 1, 0.0) }
                GradientStop { position: 1.0; color: Qt.rgba(0, 0, 0, 0.02) }
            }
        }

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
                Layout.alignment: Qt.AlignTop
                Layout.margins: Kirigami.Units.smallSpacing
                spacing: 0
                opacity: sheet.chrome
                Behavior on opacity { NumberAnimation { duration: Kirigami.Units.shortDuration } }

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
                    // As tall as a tool button, no more: the strip must not compete with the text.
                    implicitHeight: Kirigami.Units.iconSizes.smallMedium + Kirigami.Units.smallSpacing * 2
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

            QQC2.TextArea {
                id: titleField
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignTop
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Layout.rightMargin: Kirigami.Units.largeSpacing
                Layout.bottomMargin: Kirigami.Units.smallSpacing
                placeholderText: i18n("Title")
                // A long title takes at most three lines; the note below keeps its room.
                Layout.maximumHeight: Kirigami.Units.gridUnit * 4.5
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 1.35
                font.bold: true
                wrapMode: TextEdit.WrapAtWordBoundaryOrAnywhere
                leftPadding: 0
                rightPadding: 0
                topPadding: 0
                bottomPadding: 0
                color: win.ink
                placeholderTextColor: Qt.alpha(win.ink, 0.55)
                selectionColor: Qt.alpha(win.ink, 0.3)
                selectedTextColor: win.ink
                background: null
                // A title is one line of text that wraps: Enter goes on to the note, it adds no line.
                Keys.onReturnPressed: { win.editing = true; bodyField.forceActiveFocus() }
                Keys.onEnterPressed: { win.editing = true; bodyField.forceActiveFocus() }
                text: win.note.title ? win.note.title : ""
                onTextChanged: if (activeFocus) win.backend.setTitle(win.key, text)
            }

            QQC2.ScrollView {
                id: reader
                visible: !win.editing
                Layout.minimumHeight: Kirigami.Units.gridUnit * 3
                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Layout.rightMargin: Kirigami.Units.largeSpacing
                contentWidth: availableWidth
                QQC2.ScrollBar.horizontal.policy: QQC2.ScrollBar.AlwaysOff

                // Reading: the Markdown drawn.
                TextEdit {
                    width: reader.availableWidth
                    readOnly: true
                    selectByMouse: true
                    wrapMode: TextEdit.WrapAtWordBoundaryOrAnywhere
                    textFormat: TextEdit.MarkdownText
                    color: win.ink
                    selectionColor: Qt.alpha(win.ink, 0.3)
                    selectedTextColor: win.ink
                    font: Kirigami.Theme.defaultFont
                    padding: 0
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
                Layout.leftMargin: Kirigami.Units.largeSpacing
                Layout.rightMargin: Kirigami.Units.largeSpacing
                QQC2.ScrollBar.horizontal.policy: QQC2.ScrollBar.AlwaysOff

                QQC2.TextArea {
                    id: bodyField
                    placeholderText: i18n("Write in Markdown…")
                    color: win.ink
                    placeholderTextColor: Qt.alpha(win.ink, 0.55)
                    selectionColor: Qt.alpha(win.ink, 0.3)
                    selectedTextColor: win.ink
                    wrapMode: TextEdit.WrapAtWordBoundaryOrAnywhere
                    leftPadding: 0
                    rightPadding: 0
                    background: null
                    text: win.note.body ? win.note.body : ""
                    onTextChanged: if (activeFocus) win.backend.setBody(win.key, text)
                }
            }

            // The app the note came from; the corner beside it resizes.
            RowLayout {
                Layout.fillWidth: true
                Layout.margins: Kirigami.Units.smallSpacing
                Layout.leftMargin: Kirigami.Units.largeSpacing
                opacity: sheet.chrome
                Behavior on opacity { NumberAnimation { duration: Kirigami.Units.shortDuration } }
                QQC2.Label {
                    text: win.note.appName ? win.note.appName : ""
                    color: Qt.alpha(win.ink, 0.7)
                    font: Kirigami.Theme.smallFont
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
