//! The filter: what the schema offers (favourite, rating) and tags. The tags are iad's tag filter, as
//! in weave: picked tags are pills in the entry, existing tags are toggle chips under it, and several
//! groups are alternatives (a group needs all its tags). A dialog on a wide window and a full-screen
//! sheet on a narrow one.

use iced::widget::column;
use organiser_engine::Kind;
use organiser_ui::Element;
use organiser_ui::widgets::filter_sheet::filter_sheet;
use organiser_ui::widgets::schema_filters::schema_filters;
use organiser_ui::widgets::tag_filter::tag_filter;
use recipes_schema::{RECIPE, SCHEMA};

use crate::app::{App, Message};

impl App {
    pub(super) fn filter_panel(&self) -> Element<'_, Message> {
        let n = self.filter_count();
        let subtitle = match n {
            0 => "Showing every recipe".to_string(),
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
        column![
            schema_filters(kind(), &self.filters, &|_| Vec::new(), Message::Fields),
            tag_filter(
                &self.tags,
                &self.tag_groups,
                &self.tag_query,
                "A recipe needs every tag of at least one group. Press + to start another group.",
                Message::TagGroups,
                Message::TagQuery,
            ),
        ]
        .spacing(12)
        .into()
    }
}

/// The entry kind of the schema: what its filters are generated from.
pub fn kind() -> &'static Kind {
    SCHEMA
        .kinds
        .iter()
        .find(|k| k.name == RECIPE)
        .expect("the schema has recipes")
}
