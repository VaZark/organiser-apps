//! The filter: epics and tags to narrow the list by. A dialog on a wide window, a full-screen
//! sheet on a narrow one (`organiser_ui::widgets::responsive_dialog`).

use iced::widget::{column, row};
use organiser_ui::Element;
use organiser_ui::iad::prelude::*;
use organiser_ui::primitives::text::ellipsize;
use organiser_ui::widgets::filter_sheet::filter_sheet;
use organiser_ui::widgets::tag_tree;

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
            .filter(|s| self.show_closed || s.epic.status.is_open())
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

        let tag_options: Vec<ChipOption<String>> = tag_tree::ordered(&self.tags)
            .into_iter()
            .map(|(_, t)| {
                ChipOption::new(
                    t.name.clone(),
                    format!("{}  ({})", ellipsize(&t.name, 28), t.count("item")),
                )
            })
            .collect();
        let tag_chips: Element<'_, Message> = if tag_options.is_empty() {
            caption("No tags yet. Add some in a task's details.").into()
        } else {
            ChipGroup::new(tag_options, self.tag_filters.clone(), Message::ToggleTag)
                .search(&self.tag_query, Message::TagQuery)
                .placeholder("Find a tag")
                .into()
        };

        column![
            column![heading("Epic"), epic_chips, epic_actions].spacing(10),
            column![
                heading("Tags"),
                caption("A task needs at least one of the tags you pick."),
                tag_chips
            ]
            .spacing(10),
        ]
        .spacing(24)
        .into()
    }
}
