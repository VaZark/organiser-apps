//! The filter: what the schema offers (pinned, …) and the tags. A dialog on a wide window, a
//! full-screen sheet on a narrow one (`organiser_ui::widgets::responsive_dialog`). The tags dialog
//! opens over it from "Manage and group tags".

use iced::widget::column;
use notes_schema::{NOTE, SCHEMA};
use organiser_engine::Kind;
use organiser_ui::Element;
use organiser_ui::widgets::filter_sheet::filter_sheet;
use organiser_ui::widgets::schema_filters::schema_filters;
use organiser_ui::widgets::tag_filter::tag_filter;

use crate::app::{App, Message};

/// The note kind of the schema: what its filters are generated from.
pub fn kind() -> &'static Kind {
    SCHEMA
        .kinds
        .iter()
        .find(|k| k.name == NOTE)
        .expect("the schema has notes")
}

impl App {
    pub(super) fn filter_panel(&self) -> Element<'_, Message> {
        let n = self.filter_count();
        let subtitle = match n {
            0 => "Showing every note".to_string(),
            1 => "1 filter on".to_string(),
            n => format!("{n} filters on"),
        };
        let body = column![
            schema_filters(kind(), &self.filters, &|_| Vec::new(), Message::Fields),
            tag_filter(
                &self.tags,
                &self.tag_groups,
                &self.tag_query,
                "An item needs every tag of at least one group. Press + to start another group.",
                Message::TagGroups,
                Message::TagQuery,
            ),
        ]
        .spacing(24);
        filter_sheet(
            "Filter notes",
            Some(subtitle),
            body.into(),
            Message::CloseFilter,
            (n > 0).then_some(Message::ClearFilters),
            None,
        )
    }
}
