//! Notes' own page of the Preferences sheet.

use organiser_ui::Element;
use organiser_ui::components::toggle::IconToggle;
use organiser_ui::iad::prelude::*;

use crate::app::{App, Message};

/// A number of days in the trash, for the dropdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Days(pub u32);

impl std::fmt::Display for Days {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} days", self.0)
    }
}

const CHOICES: [Days; 5] = [Days(7), Days(14), Days(30), Days(60), Days(90)];

impl App {
    /// Where the notes live as files: the folder, and what can be done with it.
    fn vault_row(&self) -> Element<'_, Message> {
        let button = |icon: Icon, tip: &str, message: Message| -> Element<'_, Message> {
            IconToggle::new(icon, false)
                .tooltip(tip.to_string())
                .on_toggle(move |_| message.clone())
                .into()
        };
        let mut buttons = iced::widget::row![button(Icon::FolderOpen, "Choose a folder…", Message::ChooseVault)]
            .spacing(4)
            .align_y(iced::Alignment::Center);
        match &self.vault {
            Some(_) => {
                buttons = buttons
                    .push(button(Icon::Sync, "Sync now", Message::SyncVault))
                    .push(button(Icon::FileShow, "Show in the file manager", Message::OpenVault))
                    .push(button(Icon::WindowClose, "Keep the notes in the database alone", Message::VaultOff));
            }
            None => {
                buttons = buttons.push(button(Icon::Home, "Use Notes in the documents folder", Message::UseDefaultVault));
            }
        }
        let subtitle = match &self.vault {
            Some(v) => v.dir().display().to_string(),
            None => "Not set: the notes are kept in the database alone. Choose a folder to keep each note as a Markdown file there, with a directory for each section.".to_string(),
        };
        ActionRow::new("Notes folder")
            .subtitle(subtitle)
            .suffix(buttons)
            .into()
    }

    pub(super) fn prefs_panel(&self, wide: bool) -> Element<'_, Message> {
        let trash = EntryRow::select(
            "Days in the trash",
            &CHOICES,
            Some(Days(self.prefs.settings.notes.trash_days)),
            |d: Days| Message::PrefTrashDays(d.0),
        )
        .description("A note is deleted for good after this long in the trash.")
        .flat(true);
        let list_view = EntryRow::switch(
            "List view",
            self.prefs.settings.notes.list_view,
            Message::PrefListView,
        )
        .description("Show the notes as a list instead of cards.")
        .flat(true);
        let group = EntryRow::switch(
            "Group by section",
            self.prefs.settings.notes.group_by_folder,
            Message::PrefGroupByFolder,
        )
        .description("In All, Favorites, Archive and Trash, show the notes in groups, one for each section.")
        .flat(true);
        let group_sections = EntryRow::switch(
            "Group by section in a section",
            self.prefs.settings.notes.group_by_section,
            Message::PrefGroupBySection,
        )
        .description("On a section's page, list the notes of the sections in it under their names. Off, list them all together, each with the section it is in.")
        .flat(true);
        let show_all = EntryRow::switch(
            "Show all notes in a section",
            self.prefs.settings.notes.show_all_notes,
            Message::PrefShowAllNotes,
        )
        .description("On a section's page, list every note in it and in the sections below it. Off, the sections in it are cards on top and only its own notes are listed.")
        .flat(true);
        let page = iced::widget::column![
            list_view,
            group,
            group_sections,
            show_all,
            trash,
            self.vault_row(),
            self.sticky.row(Message::PrefStickyLink)
        ]
        .spacing(12);
        self.prefs.view(
            wide,
            "Notes",
            page.into(),
            None,
            self.tag_manager.view().map(Message::Tags),
            self.tag_manager.in_editor(),
            Message::Pref,
            Message::ClosePrefs,
        )
    }
}
