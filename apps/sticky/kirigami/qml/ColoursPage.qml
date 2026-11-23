import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

/// The paper of a sticky with no colour of its own, and the colours its menu offers.
FormCard.FormCardPage {
    id: page
    required property var backend
    title: i18n("Colours")
    readonly property var swatches: JSON.parse(backend.palette)

    FormCard.FormHeader { title: i18n("Default Colour") }
    FormCard.FormCard {
        FormCard.FormColorDelegate {
            text: i18n("Default colour")
            color: page.backend.defaultColor
            onColorChanged: {
                const hex = color.toString()
                if (hex !== page.backend.defaultColor) page.backend.chooseDefaultColor(hex)
            }
        }
    }

    FormCard.FormHeader { title: i18n("Palette") }
    FormCard.FormCard {
        Repeater {
            model: page.swatches
            delegate: FormCard.AbstractFormDelegate {
                id: row
                required property var modelData
                required property int index
                background: null
                contentItem: RowLayout {
                    spacing: Kirigami.Units.largeSpacing
                    Rectangle {
                        implicitWidth: Kirigami.Units.iconSizes.smallMedium
                        implicitHeight: implicitWidth
                        radius: width / 2
                        color: row.modelData ? row.modelData.hex : "transparent"
                        border.width: 1
                        border.color: Kirigami.Theme.disabledTextColor
                    }
                    QQC2.TextField {
                        id: hexField
                        Layout.fillWidth: true
                        text: row.modelData ? row.modelData.hex : ""
                        inputMethodHints: Qt.ImhNoPredictiveText
                        onEditingFinished: page.backend.setPaletteColor(row.index, text)
                    }
                    QQC2.ToolButton {
                        icon.name: "edit-delete"
                        enabled: page.swatches.length > 1
                        display: QQC2.AbstractButton.IconOnly
                        text: i18n("Remove Colour")
                        QQC2.ToolTip.text: text
                        QQC2.ToolTip.visible: hovered
                        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
                        onClicked: page.backend.removePaletteColor(row.index)
                    }
                }
            }
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormButtonDelegate {
            text: i18n("Add Colour")
            icon.name: "list-add"
            onClicked: page.backend.addPaletteColor()
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormButtonDelegate {
            text: i18n("Reset to Defaults")
            icon.name: "edit-reset"
            onClicked: page.backend.resetPalette()
        }
    }
}
