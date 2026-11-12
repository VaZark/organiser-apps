//! A folder's settings: its name and colour, whether its notes can be rated, and deleting it.
//! Shown as a dialog on a wide window and a full-screen sheet on a narrow one.

use iced::widget::{column, row};
use organiser_ui::Element;
use organiser_ui::components::icon_pick::icon_color_picker;
use organiser_ui::iad::prelude::*;

use crate::app::{App, Message};

impl App {
    /// The folder's settings as the content of a side drawer: a heading with a close button over
    /// the name and icon and colour, the ratings switch and deleting.
    pub(super) fn folder_settings_pane(&self) -> Element<'_, Message> {
        let Some(s) = self.settings_folder() else {
            return space().into();
        };
        let f = &s.folder;
        let name = self.folder_name.as_deref().unwrap_or(f.title.as_str());
        let head = row![
            column![
                text("Section settings").size(18),
                caption(f.title.clone()),
            ]
            .width(iced::Length::Fill),
            icon_button(Icon::WindowClose).on_press(Message::CloseFolderSettings),
        ]
        .align_y(iced::Alignment::Start);
        let body = column![
            head,
            row![
                entry("Section name", name)
                    .on_input(Message::SetFolderTitle)
                    .width(iced::Length::Fill),
                // The folder's icon and colour in one picker, at the end of its name.
                icon_color_picker(
                    f.icon.as_deref(),
                    self.folder_color(f),
                    self.folder_color_text.as_deref().unwrap_or(""),
                    22.0,
                    Message::SetFolderIcon,
                    Message::SetFolderColorText,
                    Message::SetFolderColor
                ),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            entry(
                "What it is for",
                self.folder_notes.as_deref().unwrap_or(f.notes.as_str()),
            )
            .on_input(Message::SetFolderNotes),
            // Where it is: another folder, or the top. Not itself, nor a folder in it.
            self.parent_row(f),
            EntryRow::switch("Ratings", f.rated, Message::SetFolderRated)
                .description(
                    "Give each note up to five stars. Switching this off hides them, and keeps them for when it is back on."
                )
                .flat(true),
            // The default folder is where the notes in no other folder are: it stays.
            if f.is_default {
                Element::from(caption(
                    "Unassigned holds the notes that are in no other section. It cannot be deleted.",
                ))
            } else {
                destructive_button(text("Delete section…"))
                    .on_press(Message::FolderDeleteAsk)
                    .into()
            },
        ]
        .spacing(20);
        scrolled_window(container(body).padding(16))
            .height(iced::Length::Fill)
            .into()
    }

    /// The folder it is in, to move it: a dropdown of the others (not itself or what is in it).
    fn parent_row(&self, f: &notes_schema::folder::Folder) -> Element<'_, Message> {
        if f.is_default {
            return space().into();
        }
        let current = self.parent_choices.iter().find(|c| c.id == f.parent).cloned();
        column![
            caption("In section"),
            dropdown(&self.parent_choices, current, |c: crate::app::FolderChoice| Message::SetFolderParent(c.id)),
        ]
        .spacing(4)
        .into()
    }

    /// "Delete this section?": what becomes of its notes. `None` unless one is waiting for the answer.
    pub(super) fn folder_delete_dialog(&self) -> Option<Element<'_, Message>> {
        let id = self.confirm_folder_delete?;
        let s = self.folders.iter().find(|s| s.folder.id == id)?;
        let n = s.total;
        let sub = s.subfolders;
        let what = if sub > 0 {
            "Keep them, and the sections in it, in the section it is in, or delete them all with it?"
        } else {
            "Keep them, in the section it is in, or delete them with it?"
        };
        Some(
            AlertDialog::new("Delete this section?")
                .body(format!(
                    "“{}” has {n} note{}{}. {what}",
                    s.folder.title,
                    if n == 1 { "" } else { "s" },
                    if sub > 0 { " with the sections in it" } else { "" },
                ))
                .response("Cancel", ResponseAppearance::Default, Message::FolderDeleteCancel)
                .response("Keep", ResponseAppearance::Suggested, Message::FolderDeleteKeep)
                .response("Delete all", ResponseAppearance::Destructive, Message::FolderDeleteAll)
                .into(),
        )
    }
}
