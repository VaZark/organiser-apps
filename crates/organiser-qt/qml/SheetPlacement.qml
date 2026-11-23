import QtQuick
import org.kde.kirigami as Kirigami

/// Where an OverlaySheet sits. Kirigami's own `y` depends on the sheet's `implicitHeight`, which
/// depends on `y`: a binding loop in every sheet. This places the sheet from the content's height
/// instead, so there is no cycle, and covers the whole window on mobile.
///
///     readonly property QtObject placement: SheetPlacement { popup: sheet; contentHeight: content.implicitHeight }
QtObject {
    id: root
    required property var popup
    /// The height of what the sheet shows (its header and footer are added here).
    property real contentHeight: 0

    readonly property bool ready: popup.parent !== null
    readonly property bool mobile: Kirigami.Settings.isMobile
    readonly property real parentWidth: ready ? popup.parent.width : 0
    readonly property real parentHeight: ready ? popup.parent.height : 0

    property list<QtObject> bindings: [
        Binding { target: root.popup; property: "y"; when: root.ready
            value: root.mobile ? 0 : Math.round(Math.max(Kirigami.Units.gridUnit * 3,
                (root.parentHeight - root.contentHeight - Kirigami.Units.gridUnit * 8) / 2)) },
        Binding { target: root.popup; property: "x"; value: 0; when: root.ready && root.mobile },
        Binding { target: root.popup; property: "width"; value: root.parentWidth; when: root.ready && root.mobile },
        Binding { target: root.popup; property: "height"; value: root.parentHeight; when: root.ready && root.mobile }
    ]
}
