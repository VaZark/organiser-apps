//! Todo's own page of the Preferences sheet.

use iced::widget::{column, row};
use organiser_core::settings::StartView;
use organiser_ui::Element;
use organiser_ui::components::toggle::IconToggle;
use organiser_ui::iad::prelude::*;
use organiser_ui::widgets::preferences::Page;

use crate::app::{App, Message};

impl App {
    /// Where the tasks live as files: the folder, and what can be done with it.
    fn vault_row(&self) -> Element<'_, Message> {
        let button = |icon: Icon, tip: &str, message: Message| -> Element<'_, Message> {
            IconToggle::new(icon, false)
                .tooltip(tip.to_string())
                .on_toggle(move |_| message.clone())
                .into()
        };
        let mut buttons = row![button(
            Icon::FolderOpen,
            "Choose a folder…",
            Message::ChooseVault
        )]
        .spacing(4)
        .align_y(iced::Alignment::Center);
        match &self.vault {
            Some(_) => {
                buttons = buttons
                    .push(button(Icon::Sync, "Load from folder", Message::SyncVault))
                    .push(button(
                        Icon::FileShow,
                        "Show in the file manager",
                        Message::OpenVault,
                    ))
                    .push(button(
                        Icon::WindowClose,
                        "Keep the tasks in the database alone",
                        Message::VaultOff,
                    ));
            }
            None => {
                buttons = buttons.push(button(
                    Icon::Home,
                    "Use Todo in the documents folder",
                    Message::UseDefaultVault,
                ));
            }
        }
        let subtitle = match &self.vault {
            Some(v) => v.dir().display().to_string(),
            None => "Not set: the tasks are kept in the database alone. Choose a folder to keep each task as a Markdown file there, a directory for each epic and one for each task with subtasks.".to_string(),
        };
        ActionRow::new("Tasks folder")
            .subtitle(subtitle)
            .suffix(buttons)
            .into()
    }

    /// The folders page of Preferences: the Tasks folder, and the folders imported into it. An
    /// import is a link: nothing is copied, and removing it leaves the folder as it was.
    fn folders_page(&self) -> Element<'_, Message> {
        let mut imports = preferences_group().title("Imported folders").description(
            "Folders kept elsewhere that show up here as epics. Each is a link inside the Tasks folder: nothing is copied, and removing one never deletes the folder.",
        );
        let list = self.vault.as_ref().map(|v| v.imports()).unwrap_or_default();
        for (name, target) in list {
            let remove: Element<'_, Message> = IconToggle::new(Icon::Delete, false)
                .tooltip("Remove the link (the folder stays)".to_string())
                .on_toggle({
                    let name = name.clone();
                    move |_| Message::UnlinkFolder(name.clone())
                })
                .into();
            imports = imports.push(
                ActionRow::new(name)
                    .subtitle(target.display().to_string())
                    .suffix(remove),
            );
        }
        // Without a Tasks folder there is nothing to link into: the row only says so.
        let mut import_row = ActionRow::new("Import a folder…");
        import_row = if self.vault.is_some() {
            import_row
                .subtitle("Choose an existing folder of Markdown tasks to link in.")
                .suffix(
                    IconToggle::new(Icon::FolderOpen, false)
                        .tooltip("Import a folder…".to_string())
                        .on_toggle(|_| Message::ImportFolder),
                )
        } else {
            import_row.subtitle("Set a Tasks folder first.")
        };
        column![self.vault_row(), imports.push(import_row)]
            .spacing(12)
            .into()
    }

    pub(super) fn prefs_panel(&self, wide: bool) -> Element<'_, Message> {
        let todo = &self.prefs.settings.todo;
        let page = column![
            EntryRow::select(
                "Default view",
                &StartView::ALL,
                Some(todo.start_view),
                Message::PrefStartView,
            )
            .description("What Todo opens on: notes, the kanban board or the list.")
            .flat(true),
            EntryRow::switch(
                "Show finished tasks",
                todo.show_closed,
                Message::PrefShowClosed
            )
            .description("Start with finished tasks shown, in every view.")
            .flat(true),
            EntryRow::switch(
                "Group by epic",
                todo.group_by_epic,
                Message::PrefGroupByEpic
            )
            .description("In the views of every task, group them by epic after the pinned ones, the tasks in no epic last.")
            .flat(true),
            EntryRow::switch(
                "Group by section",
                todo.group_by_section,
                Message::PrefGroupBySection
            )
            .description("On an epic's page, group its tasks by section.")
            .flat(true),
            EntryRow::switch(
                "Show subtasks",
                todo.show_subtasks,
                Message::PrefShowSubtasks
            )
            .description("List subtasks as entries of their own in every view. Off, only top-level tasks are listed.")
            .flat(true),
            EntryRow::switch(
                "Open details in drawer",
                todo.details_in_drawer,
                Message::PrefDetailsDrawer
            )
            .description("Show a task's details beside the list. On narrow windows, or when off, they open as a full page.")
            .flat(true),
            self.sticky.row(Message::PrefStickyLink),
        ]
        .spacing(12);
        self.prefs.view(
            wide,
            "Todo",
            page.into(),
            vec![
                (Page::Extra, "Statuses", Icon::Palette, self.statuses_page()),
                (
                    Page::Folders,
                    "Folders",
                    Icon::FolderOpen,
                    self.folders_page(),
                ),
            ],
            self.tag_manager.view().map(Message::Tags),
            self.tag_manager.in_editor(),
            Message::Pref,
            Message::ClosePrefs,
        )
    }
}
