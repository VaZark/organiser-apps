//! A list's settings: its name, and which extras its entries have. Shown as a dialog on a wide
//! window and a full-screen sheet on a narrow one.

use iced::widget::column;
use organiser_ui::Element;
use organiser_ui::iad::prelude::*;
use organiser_ui::widgets::filter_sheet::filter_sheet;

use crate::app::{App, Message};

impl App {
    pub(super) fn settings_panel(&self) -> Element<'_, Message> {
        let Some(s) = self.current() else {
            return space().into();
        };
        let l = &s.list;
        let body = column![
            entry("List name", l.title.as_str()).on_input(Message::SetListTitle),
            column![
                EntryRow::switch("Ratings", l.rated, Message::SetRated),
                caption("Give each entry up to five stars. Switching this off removes the ratings."),
            ]
            .spacing(4),
            column![
                EntryRow::switch("Status", l.tracked, Message::SetTracked),
                caption("Track each entry as planned, in progress or done. Switching this off removes the statuses."),
            ]
            .spacing(4),
            column![
                destructive_button(text(if self.confirm_delete {
                    format!("Press again to delete the list and its {} entries", s.entries)
                } else {
                    "Delete list…".to_string()
                }))
                .on_press(Message::DeleteList),
            ],
        ]
        .spacing(20);
        filter_sheet(
            "List settings",
            Some(l.title.clone()),
            body.into(),
            Message::CloseSettings,
            None,
            None,
        )
    }
}
