//! Todo's own page of the Preferences sheet.

use iced::widget::column;
use organiser_core::settings::StartView;
use organiser_ui::Element;
use organiser_ui::iad::prelude::*;

use crate::app::{App, Message};

impl App {
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
            Some(("Statuses", self.statuses_page())),
            self.tag_manager.view().map(Message::Tags),
            self.tag_manager.in_editor(),
            Message::Pref,
            Message::ClosePrefs,
        )
    }
}
