import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

/// A tag as a small rounded label; `tint` is the tag's own colour (user data).
Rectangle {
    id: chip
    property string text
    property color tint: Kirigami.Theme.highlightColor
    implicitHeight: label.implicitHeight + Kirigami.Units.smallSpacing
    implicitWidth: label.implicitWidth + Kirigami.Units.largeSpacing
    radius: height / 2
    color: Qt.alpha(tint, 0.18)
    border.color: Qt.alpha(tint, 0.5)
    QQC2.Label {
        id: label
        anchors.centerIn: parent
        text: chip.text
        font: Kirigami.Theme.smallFont
    }
}
