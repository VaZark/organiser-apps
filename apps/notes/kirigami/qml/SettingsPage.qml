import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard

/// Configure Notes: how notes are shown, the trash, the folder the notes are kept in, and tags.
FormCard.FormCardPage {
    id: page
    required property var backend
    signal openTags()

    readonly property var prefs: JSON.parse(backend.prefs)
    property bool importAsLink: false

    title: i18n("Configure Notes")

    FolderDialog {
        id: folderDialog
        title: i18n("Choose the Notes Folder")
        onAccepted: page.backend.setVault(selectedFolder.toString())
    }
    FileDialog {
        id: importDialog
        title: i18n("Import Markdown Files")
        fileMode: FileDialog.OpenFiles
        nameFilters: [i18n("Markdown files (*.md *.markdown)"), i18n("All files (*)")]
        onAccepted: page.backend.importFiles(JSON.stringify(selectedFiles.map(u => u.toString())), page.importAsLink)
    }
    FolderDialog {
        id: importFolderDialog
        title: i18n("Import a Folder of Markdown Files")
        onAccepted: page.backend.importFiles(JSON.stringify([selectedFolder.toString()]), page.importAsLink)
    }

    FormCard.FormHeader { title: i18n("Display") }
    FormCard.FormCard {
        FormCard.FormSwitchDelegate {
            text: i18n("Show notes as a list")
            description: i18n("A list of rows instead of cards.")
            checked: page.prefs.listView === true
            onToggled: page.backend.setPref("listView", checked)
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormSwitchDelegate {
            text: i18n("Group by folder")
            description: i18n("In All Notes, Favorites, Archive and Trash, group the notes by folder.")
            checked: page.prefs.groupByFolder === true
            onToggled: page.backend.setPref("groupByFolder", checked)
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormSwitchDelegate {
            text: i18n("Group by subfolder in a folder")
            description: i18n("On a folder's page, list the notes of the folders in it under their names. Off, list them together, each with its folder.")
            checked: page.prefs.groupBySection === true
            onToggled: page.backend.setPref("groupBySection", checked)
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormSwitchDelegate {
            text: i18n("Show all notes of a folder")
            description: i18n("On a folder's page, list every note in it and in the folders below it.")
            checked: page.prefs.showAllNotes === true
            onToggled: page.backend.setPref("showAllNotes", checked)
        }
    }

    FormCard.FormHeader { title: i18n("Trash") }
    FormCard.FormCard {
        FormCard.FormSpinBoxDelegate {
            label: i18n("Days in the trash")
            from: 1
            to: 3650
            value: page.prefs.trashDays
            onValueChanged: if (value !== page.prefs.trashDays) page.backend.setTrashDays(value)
        }
    }

    FormCard.FormHeader { title: i18n("Notes Folder") }
    FormCard.FormCard {
        FormCard.FormTextDelegate {
            text: page.prefs.vaultDir !== "" ? page.prefs.vaultDir : i18n("None")
            description: page.prefs.vaultDir !== ""
                ? i18n("Each note is a Markdown file here, with a directory for each folder.")
                : i18n("The notes are kept in the database alone. Choose a folder to keep each note as a Markdown file.")
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormButtonDelegate {
            text: i18n("Choose Folder…")
            icon.name: "folder-open"
            onClicked: folderDialog.open()
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormButtonDelegate {
            text: i18n("Use Notes in Documents")
            icon.name: "folder-documents"
            onClicked: page.backend.useDefaultVault()
        }
        FormCard.FormDelegateSeparator { visible: page.prefs.vaultDir !== "" }
        FormCard.FormButtonDelegate {
            visible: page.prefs.vaultDir !== ""
            text: i18n("Reload from Folder")
            icon.name: "view-refresh"
            onClicked: page.backend.syncVault()
        }
        FormCard.FormDelegateSeparator { visible: page.prefs.vaultDir !== "" }
        FormCard.FormButtonDelegate {
            visible: page.prefs.vaultDir !== ""
            text: i18n("Show in File Manager")
            icon.name: "document-open-folder"
            onClicked: Qt.openUrlExternally("file://" + page.prefs.vaultDir)
        }
        FormCard.FormDelegateSeparator { visible: page.prefs.vaultDir !== "" }
        FormCard.FormButtonDelegate {
            visible: page.prefs.vaultDir !== ""
            text: i18n("Stop Using the Folder")
            description: i18n("The notes stay in the database; the files stay where they are.")
            icon.name: "edit-clear"
            onClicked: page.backend.setVault("")
        }
    }

    FormCard.FormHeader { title: i18n("Import"); visible: page.prefs.vaultDir !== "" }
    FormCard.FormCard {
        visible: page.prefs.vaultDir !== ""
        FormCard.FormComboBoxDelegate {
            text: i18n("Bring files in as")
            model: [i18n("Copies"), i18n("Links to the originals")]
            onActivated: page.importAsLink = currentIndex === 1
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormButtonDelegate {
            text: i18n("Import Files…")
            icon.name: "document-import"
            onClicked: importDialog.open()
        }
        FormCard.FormDelegateSeparator {}
        FormCard.FormButtonDelegate {
            text: i18n("Import Folder…")
            icon.name: "folder-import"
            onClicked: importFolderDialog.open()
        }
    }

    FormCard.FormHeader { title: i18n("Tags") }
    FormCard.FormCard {
        FormCard.FormButtonDelegate {
            text: i18n("Manage Tags…")
            description: i18n("Names, colors and which tags are inside others")
            icon.name: "tag"
            onClicked: page.openTags()
        }
    }
}
