//! The selected entry: its title, tags, the extras its list offers (a rating, a status), the list
//! it belongs to, and notes or an opinion. Every edit is saved as it is made.

use iced::widget::{column, row, text_editor};
use iced::{Alignment, Length};
use organiser_ui::Element;
use organiser_ui::components::heart::heart;
use organiser_ui::iad::prelude::*;
use organiser_ui::widgets::tag_editor::tag_editor;

use crate::app::{App, Message, StatusChoice};

fn field<'a>(label: &'a str, control: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    column![caption(label), control.into()]
        .spacing(4)
        .width(Length::Fill)
        .into()
}

impl App {
    pub(super) fn details_pane(&self) -> Element<'_, Message> {
        let Some(d) = &self.details else {
            return space().into();
        };
        let e = &d.entry;
        let list = self.list_of(e);

        let heart = heart(e.favorite, Message::DFavorite(!e.favorite));
        let mut col = column![
            // Closing the details lives in the pane, so it works however they are shown.
            row![
                tooltip(
                    icon_button(Icon::GoPrevious).on_press(Message::CloseDetails),
                    "Back",
                    TooltipPosition::Bottom
                ),
                caption(list.map_or("Entry".to_string(), |l| l.title.clone())),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            row![
                entry("Title", d.title.as_str())
                    .on_input(Message::DTitle)
                    .width(Length::Fill),
                heart
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            tag_editor(
                &e.tags,
                d.adding_tag,
                &d.tag_text,
                &self.tag_names,
                Message::DTagStart,
                Message::DTagText,
                &|t| Message::DTagAdd(t),
                &|t| Message::DTagRemove(t),
            ),
        ]
        .spacing(14)
        .width(Length::Fill);

        // The extras are the list's choice: nothing to rate where ratings are off.
        if list.is_some_and(|l| l.rated) {
            let mut stars = row![].spacing(2).align_y(Alignment::Center);
            for n in 1..=5u8 {
                let on = e.rating.is_some_and(|r| n <= r);
                // Pressing the star that is already the rating clears it.
                let next = if e.rating == Some(n) { None } else { Some(n) };
                stars = stars.push(
                    flat_button(text(if on { "★" } else { "☆" }).size(22))
                        .on_press(Message::DRating(next)),
                );
            }
            col = col.push(field("Rating", stars));
        }
        if list.is_some_and(|l| l.tracked) {
            col = col.push(field(
                "Status",
                dropdown(
                    &StatusChoice::ALL,
                    Some(StatusChoice(e.status)),
                    Message::DStatus,
                ),
            ));
        }
        let here = self
            .list_choices
            .iter()
            .find(|c| c.id == e.list_id)
            .cloned();
        col = col
            .push(field(
                "List",
                dropdown(&self.list_choices, here, Message::DMove),
            ))
            .push(field(
                "Notes and opinion",
                text_view(&d.notes)
                    .on_action(|a: text_editor::Action| Message::DNotes(a))
                    .placeholder("What did you think? Anything worth remembering.")
                    .min_lines(8)
                    .max_lines(20),
            ));
        if let Some(err) = &d.error {
            col = col.push(tag(err.clone(), Role::Error));
        }
        col = col.push(destructive_button(text("Delete")).on_press(Message::DDelete));
        scrolled_window(clamp(
            container(col).padding(16),
            organiser_ui::widgets::details_dock::READABLE_WIDTH,
        ))
        .height(Length::Fill)
        .into()
    }
}
