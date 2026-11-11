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
                "Start on",
                &StartView::ALL,
                Some(todo.start_view),
                Message::PrefStartView,
            )
            .description("What Todo opens on: the kanban board or the list.")
            .flat(true),
            EntryRow::switch(
                "Show finished tasks",
                todo.show_closed,
                Message::PrefShowClosed
            )
            .description("Start with finished tasks shown, on the board and in the list.")
            .flat(true),
        ]
        .spacing(12);
        self.prefs.view(
            wide,
            "Todo",
            page.into(),
            Some(("Statuses", self.statuses_page())),
            self.tag_manager.view().map(Message::Tags),
            self.tag_manager.editing().is_some(),
            Message::Pref,
            Message::ClosePrefs,
        )
    }
}
