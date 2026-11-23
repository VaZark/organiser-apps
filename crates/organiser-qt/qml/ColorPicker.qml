import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

/// An item's colour, the same in every app: quick-pick swatches, "No Color", and any `#rrggbb`
/// typed. `presets` is `[{name, hex}]` from the backend (the shared list in organiser-core), `color`
/// the current one (empty: none). `picked(hex)` fires for a swatch, "No Color" (empty) or a typed
/// value; the owner stores it and the backend refuses what is not a colour.
ColumnLayout {
    id: picker

    property var presets: []
    property string color: ""
    signal picked(string hex)

    spacing: Kirigami.Units.smallSpacing

    Flow {
        Layout.fillWidth: true
        spacing: Kirigami.Units.smallSpacing
        Repeater {
            model: picker.presets
            delegate: QQC2.AbstractButton {
                id: swatch
                required property var modelData
                readonly property bool current: picker.color.toLowerCase() === modelData.hex.toLowerCase()
                implicitWidth: Kirigami.Units.gridUnit * 1.4
                implicitHeight: implicitWidth
                text: modelData.name
                QQC2.ToolTip.text: text
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                onClicked: picker.picked(modelData.hex)
                contentItem: null
                background: Rectangle {
                    radius: width / 2
                    color: swatch.modelData.hex
                    border.width: swatch.current ? 3 : 1
                    border.color: swatch.current ? Kirigami.Theme.highlightColor
                        : Kirigami.ColorUtils.linearInterpolation(Kirigami.Theme.backgroundColor, Kirigami.Theme.textColor, 0.25)
                }
            }
        }
        QQC2.ToolButton {
            icon.name: "edit-clear"
            text: i18n("No Color")
            display: QQC2.AbstractButton.IconOnly
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
            QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
            enabled: picker.color !== ""
            onClicked: { hex.text = ""; picker.picked("") }
        }
    }
    QQC2.TextField {
        id: hex
        Layout.fillWidth: true
        placeholderText: i18n("Custom color: #rrggbb")
        maximumLength: 7
        text: picker.color
        onAccepted: picker.picked(text.trim())
        // A pick elsewhere (swatch, another view of the same item) shows here.
        Connections { target: picker; function onColorChanged() { if (!hex.activeFocus) hex.text = picker.color } }
    }
}
