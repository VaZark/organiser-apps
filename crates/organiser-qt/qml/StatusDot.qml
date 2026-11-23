import QtQuick
import org.kde.kirigami as Kirigami

/// The dot of a status, in the status's own colour.
Rectangle {
    property color tint: Kirigami.Theme.disabledTextColor
    implicitWidth: Kirigami.Units.gridUnit * 0.6
    implicitHeight: implicitWidth
    radius: width / 2
    color: tint
}
