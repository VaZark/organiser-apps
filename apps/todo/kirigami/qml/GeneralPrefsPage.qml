import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import org.kde.kirigami as Kirigami
import org.organiser.todo

Kirigami.ScrollablePage {
    id: page
    required property var backend
    title: i18n("General")

    FolderDialog {
        id: folderDialog
        title: i18n("Choose the tasks folder")
        onAccepted: page.backend.setVault(selectedFolder.toString())
    }

    Kirigami.FormLayout {
        QQC2.CheckBox {
            Kirigami.FormData.label: i18n("Lists:")
            text: i18n("Start with finished tasks shown")
            checked: page.backend.showClosed
            onToggled: { page.backend.showClosed = checked; page.backend.applyFilters(); page.backend.savePreferences() }
        }
        QQC2.CheckBox {
            text: i18n("List subtasks as entries of their own")
            checked: page.backend.showSubtasks
            onToggled: { page.backend.showSubtasks = checked; page.backend.applyFilters(); page.backend.savePreferences() }
        }
        QQC2.ComboBox {
            Kirigami.FormData.label: i18n("Open on:")
            textRole: "text"
            valueRole: "value"
            model: [
                { text: i18n("Notes"), value: "notes" },
                { text: i18n("Board"), value: "board" },
                { text: i18n("List"), value: "list" },
                { text: i18n("Matrix"), value: "quadrants" }
            ]
            currentIndex: Math.max(0, model.findIndex(m => m.value === page.backend.startView))
            onActivated: { page.backend.startView = currentValue; page.backend.savePreferences() }
        }
        Kirigami.Separator { Kirigami.FormData.isSection: true; Kirigami.FormData.label: i18n("Tasks folder") }
        QQC2.Label {
            Kirigami.FormData.label: i18n("Folder:")
            text: page.backend.vaultDir !== "" ? page.backend.vaultDir : i18n("None: tasks stay in the database only")
            wrapMode: Text.Wrap
            Layout.maximumWidth: Kirigami.Units.gridUnit * 24
        }
        RowLayout {
            QQC2.Button { text: i18n("Choose folder…"); icon.name: "folder-open"; onClicked: folderDialog.open() }
            QQC2.Button { text: i18n("Sync now"); icon.name: "view-refresh"; enabled: page.backend.vaultDir !== ""; onClicked: page.backend.syncVault() }
            QQC2.Button { text: i18n("Stop using"); icon.name: "edit-clear"; enabled: page.backend.vaultDir !== ""; onClicked: page.backend.setVault("") }
        }
        QQC2.Label {
            Kirigami.FormData.label: i18n("Appearance:")
            text: i18n("Follows your KDE colour scheme and style.")
        }
    }
}
