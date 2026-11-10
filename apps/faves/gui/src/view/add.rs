//! Adding an entry: a title, then Add. A dialog on a wide window, a full-screen sheet on a narrow
//! one. The entry takes focus as it opens.

use iced::widget::column;
use organiser_ui::Element;
use organiser_ui::iad::prelude::*;
use organiser_ui::primitives::text::ellipsize;
use organiser_ui::widgets::filter_sheet::filter_sheet;

use crate::app::{App, Message};

const ADD_ENTRY_ID: &str = "faves.add-entry";

impl App {
    pub(super) fn add_panel(&self) -> Element<'_, Message> {
        let Some(s) = self.current() else {
            return space().into();
        };
        let body = column![
            FocusScope::new(
                entry("Title", self.quick.as_str())
                    .id(ADD_ENTRY_ID)
                    .on_input(Message::QuickText)
                    .on_submit(Message::QuickSubmit),
            )
            .trap(false)
            .initial_focus(ADD_ENTRY_ID),
            caption("You can add the notes, tags and rating afterwards."),
        ]
        .spacing(10);
        filter_sheet(
            &format!("Add to {}", ellipsize(&s.list.title, 30)),
            None,
            body.into(),
            Message::CloseAdd,
            None,
            Some(("Add", Message::QuickSubmit)),
        )
    }
}
