//! A section's settings, in a drawer at the side of the folder's page: its name, colour and
//! description, and deleting it (its notes stay in the folder).

use iced::widget::{column, row};
use iced::{Alignment, Length};
use organiser_ui::Element;
use organiser_ui::components::color_pick::color_swatch_rgb;
use organiser_ui::iad::prelude::*;

use crate::app::{App, Message, paint};

impl App {
    pub(super) fn section_settings_pane(&self) -> Element<'_, Message> {
        let Some(id) = self.section_settings else {
            return space().into();
        };
        let Some(s) = self
            .folder_sections
            .values()
            .flatten()
            .find(|s| s.section.id == id)
        else {
            return space().into();
        };
        let sec = &s.section;
        let name = self.section_name.as_deref().unwrap_or(sec.title.as_str());
        let notes = self.section_notes.as_deref().unwrap_or(sec.notes.as_str());
        let color = sec
            .color
            .map_or(organiser_ui::components::color_pick::UNSET, paint);
        let head = row![
            column![text("Section settings").size(18), caption(sec.title.clone())]
                .width(Length::Fill),
            icon_button(Icon::WindowClose).on_press(Message::CloseSectionSettings),
        ]
        .align_y(Alignment::Start);
        let body = column![
            head,
            row![
                entry("Section name", name)
                    .on_input(Message::SetSectionTitle)
                    .width(Length::Fill),
                // The section's colour, at the end of its name.
                color_swatch_rgb(
                    color,
                    self.section_color_text.as_deref().unwrap_or(""),
                    22.0,
                    Message::SetSectionColorText,
                    Message::SetSectionColor
                ),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            entry("What it is for", notes).on_input(Message::SetSectionNotes),
            caption(format!("{} notes in this section", s.notes)),
            destructive_button(text("Delete section")).on_press(Message::DeleteSection),
        ]
        .spacing(20);
        scrolled_window(container(body).padding(16))
            .height(Length::Fill)
            .into()
    }
}
