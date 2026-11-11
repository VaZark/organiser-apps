//! The filter: epics and tags to narrow the list by. A dialog on a wide window, a full-screen
//! sheet on a narrow one (`organiser_ui::widgets::responsive_dialog`).

use iced::widget::{column, row};
use organiser_engine::Kind;
use organiser_ui::Element;
use organiser_ui::components::dot::color_from_hex;
use organiser_ui::iad::prelude::*;
use organiser_ui::primitives::text::ellipsize;
use organiser_ui::widgets::filter_sheet::filter_sheet;
use organiser_ui::widgets::schema_filters::{Choice, schema_filters};
use organiser_ui::widgets::tag_filter::tag_filter;
use todo_schema::{SCHEMA, TASK};

use crate::app::{App, Message};

impl App {
    pub(super) fn filter_panel(&self) -> Element<'_, Message> {
        let n = self.filter_count();
        let subtitle = match n {
            0 => "Showing every task".to_string(),
            1 => "1 filter on".to_string(),
            n => format!("{n} filters on"),
        };
        filter_sheet(
            "Filter tasks",
            Some(subtitle),
            self.filter_body(),
            Message::CloseFilter,
            (n > 0).then_some(Message::ClearFilters),
            None,
        )
    }

    fn filter_body(&self) -> Element<'_, Message> {
        let epics: Vec<ChipOption<i64>> = self
            .epics
            .iter()
            .filter(|s| self.show_closed || self.statuses.is_open(s.epic.status))
            .map(|s| {
                ChipOption::new(
                    s.epic.id,
                    format!(
                        "{}  ({}/{})",
                        ellipsize(&s.epic.title, 28),
                        s.total - s.open,
                        s.total
                    ),
                )
            })
            .collect();
        let selected_epic: Vec<i64> = self.epic_filter.into_iter().collect();

        let mut epic_actions = row![
            flat_button(row![Icon::ListAdd.view(16.0), text("New epic")].spacing(6))
                .on_press(Message::NewEpic)
        ]
        .spacing(8);
        if let Some(id) = self.epic_filter {
            epic_actions = epic_actions.push(
                flat_button(row![Icon::Edit.view(16.0), text("Edit epic")].spacing(6))
                    .on_press(Message::EditEpic(id)),
            );
        }
        let epic_chips: Element<'_, Message> = if epics.is_empty() {
            caption("No epics yet. An epic groups related tasks.").into()
        } else {
            ChipGroup::new(epics, selected_epic, Message::ToggleEpic).into()
        };

        let generated = schema_filters(
            kind(),
            &self.filters,
            &|f| match f.name {
                // A status is one of the person's own, in its colour.
                "status" => self
                    .statuses
                    .all()
                    .iter()
                    .map(|s| Choice {
                        value: serde_json::Value::from(s.id),
                        label: s.name.clone(),
                        color: s.color.as_deref().and_then(color_from_hex),
                    })
                    .collect(),
                _ => Vec::new(),
            },
            Message::Fields,
        );
        column![
            EntryRow::new(
                "Epic",
                EntryInput::Custom(column![epic_chips, epic_actions].spacing(10).into())
            )
            .description("An epic groups related tasks.")
            .flat(true),
            generated,
            tag_filter(
                &self.tags,
                &self.tag_groups,
                &self.tag_query,
                "A task needs every tag of at least one group. Press + to start another group.",
                Message::TagGroups,
                Message::TagQuery,
            ),
        ]
        .spacing(24)
        .into()
    }
}

/// The task kind of the schema: what its filters are generated from.
pub fn kind() -> &'static Kind {
    SCHEMA
        .kinds
        .iter()
        .find(|k| k.name == TASK)
        .expect("the schema has tasks")
}
