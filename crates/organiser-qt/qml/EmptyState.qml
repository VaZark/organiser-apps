import QtQuick
import org.kde.kirigami as Kirigami

/// Shown where a list has nothing to list.
Kirigami.PlaceholderMessage {
    anchors.centerIn: parent
    width: parent.width - Kirigami.Units.gridUnit * 4
}
