import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.dateandtime as Dates

/// A date as the system locale writes it (`29/11/2026`, `11/29/2026`, ...): type it, or pick it from the
/// calendar button. `text` is the stored date, always ISO `YYYY-MM-DD` (empty is none);
/// `committed(iso)` fires only for a real date or for empty (clear), never for half-typed text,
/// which is flagged and put back when the field loses focus.
RowLayout {
    id: field

    property string text: ""
    property alias placeholderText: input.placeholderText
    /// The locale to read and write dates in; the system's unless a test sets another.
    property alias locale: dates.locale
    /// Show the Clear action in the calendar popup.
    property bool clearable: true
    signal committed(string iso)

    LocaleDates { id: dates }

    /// True for the empty string and for a real date in the locale's format.
    function isValid(s) { return dates.parseInput(s) !== null }
    function commit(iso) {
        field.text = iso
        field.committed(iso)
    }

    spacing: Kirigami.Units.smallSpacing

    QQC2.TextField {
        id: input
        Layout.fillWidth: true
        placeholderText: dates.format.toUpperCase()
        text: dates.show(field.text)
        inputMethodHints: Qt.ImhDate | Qt.ImhPreferNumbers
        // Flagged while it is neither empty nor a whole, real date.
        Kirigami.Theme.colorSet: Kirigami.Theme.View
        color: field.isValid(text) ? Kirigami.Theme.textColor : Kirigami.Theme.negativeTextColor
        onEditingFinished: {
            const iso = dates.parseInput(text)
            if (iso === null) {
                text = dates.show(field.text) // refuse: keep the old date
            } else if (iso !== field.text) {
                field.commit(iso)
            }
        }
        // Keep it in step when the stored date changes from elsewhere.
        Connections {
            target: field
            function onTextChanged() { if (!input.activeFocus) input.text = dates.show(field.text) }
        }
    }

    QQC2.ToolButton {
        icon.name: "view-calendar"
        text: i18n("Pick a date")
        display: QQC2.AbstractButton.IconOnly
        QQC2.ToolTip.text: text
        QQC2.ToolTip.visible: hovered
        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
        onClicked: {
            popup.value = dates.fromIso(field.text) || new Date()
            popup.open()
        }
    }

    Dates.DatePopup {
        id: popup
        parent: QQC2.Overlay.overlay
        anchors.centerIn: parent
        resettable: field.clearable
        onAccepted: field.commit(dates.toIso(value))
        onReset: field.commit("")
    }
}
