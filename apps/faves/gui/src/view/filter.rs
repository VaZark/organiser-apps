//! The filter: favourites only, a minimum rating, and tags. The tags are iad's tag filter, as in
//! weave: picked tags are pills in the entry, existing tags are toggle chips under it, and several
//! groups are alternatives (a group needs all its tags). A dialog on a wide window and a full-screen
//! sheet on a narrow one.

use iced::widget::{column, row};
use organiser_ui::Element;
use organiser_ui::iad::prelude::*;
use organiser_ui::iad::widgets::TagFilter;
use organiser_ui::primitives::text::ellipsize;
use organiser_ui::widgets::filter_sheet::filter_sheet;

use crate::app::{App, Message};

impl App {
    pub(super) fn filter_panel(&self) -> Element<'_, Message> {
        let n = self.filter_count();
        let subtitle = match n {
            0 => "Showing every entry".to_string(),
            1 => "1 filter on".to_string(),
            n => format!("{n} filters on"),
        };
        filter_sheet(
            "Filter",
            Some(subtitle),
            self.filter_body(),
            Message::CloseFilter,
            (n > 0).then_some(Message::ClearFilters),
            None,
        )
    }

    fn filter_body(&self) -> Element<'_, Message> {
        // The stars: pressing the one that is already the minimum clears it.
        let mut stars = row![].spacing(2).align_y(iced::Alignment::Center);
        for n in 1..=5u8 {
            let on = self.min_rating.is_some_and(|m| n <= m);
            let next = if self.min_rating == Some(n) {
                None
            } else {
                Some(n)
            };
            stars = stars.push(
                flat_button(text(if on { "★" } else { "☆" }).size(24))
                    .on_press(Message::MinRating(next)),
            );
        }
        let rating_note = match self.min_rating {
            Some(1) => "1 star or more".to_string(),
            Some(n) => format!("{n} stars or more"),
            None => "Any rating, rated or not".to_string(),
        };

        let options: Vec<ChipOption<String>> = self
            .tags
            .iter()
            .map(|t| {
                ChipOption::new(
                    t.name.clone(),
                    format!("{}  ({})", ellipsize(&t.name, 28), t.count("item")),
                )
            })
            .collect();
        let tags: Element<'_, Message> = if options.is_empty() {
            caption("No tags yet. Add some in an entry's details.").into()
        } else {
            TagFilter::new(
                &self.tag_groups,
                options,
                &self.tag_query,
                Message::TagGroups,
                Message::TagQuery,
            )
            .multiple(true)
            .label("Tags")
            .placeholder("Find a tag")
            .flat(true)
            .into()
        };

        column![
            EntryRow::switch("Only favourites", self.fav_only, Message::FavOnly),
            column![heading("Rating"), stars, caption(rating_note)].spacing(6),
            column![
                heading("Tags"),
                caption("An entry needs every tag in a group. Several groups are alternatives."),
                tags
            ]
            .spacing(8),
        ]
        .spacing(22)
        .into()
    }
}
