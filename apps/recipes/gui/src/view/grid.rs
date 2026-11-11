//! The recipes: a masonry grid of cards as Google Keep lays out notes, or one compact line per
//! entry. Above them, the sort, an add button and the filters that are on.

use iced::widget::{column, row};
use iced::{Alignment, Length};
use organiser_ui::Element;
use organiser_ui::components::card::card;
use organiser_ui::components::chip::{color_of, hashtag, more};
use organiser_ui::components::empty::empty;
use organiser_ui::components::heart::heart;
use organiser_ui::components::row::selectable;
use organiser_ui::components::tag_groups::tag_groups;
use organiser_ui::components::tag_query::count as tag_count;
use organiser_ui::components::toggle::IconToggle;
use organiser_ui::iad::prelude::*;
use organiser_ui::iad::primitives::tag_groups::GroupEvent;
use organiser_ui::primitives::tags::collapse;
use organiser_ui::primitives::text::{ellipsize, snippet};
use organiser_ui::widgets::schema_filters::active_filters;
use organiser_ui::widgets::tag_filter::tag_filter;
use recipes_schema::model::Recipe;

use crate::app::{App, Message, View};

/// The narrowest a card is allowed to get before the grid drops a column.
const CARD_WIDTH: f32 = 270.0;

/// `★★★★☆`
pub fn stars(rating: u8) -> String {
    let r = rating.min(5) as usize;
    format!("{}{}", "★".repeat(r), "☆".repeat(5 - r))
}

impl App {
    /// The top of the recipes page: at the right edge the page's actions (sort, search). Adding a recipe is the "+" at
    /// the top left of the window, as on the log page. Search opens the text and the tags to filter by.
    fn search_row(&self) -> Element<'_, Message> {
        if !self.search.is_open() {
            return row![
                space().width(Length::Fill),
                self.sort_button(),
                tooltip(
                    icon_button(Icon::Search).on_press(Message::ToggleSearch),
                    "Search and filter by tag (Ctrl+F)",
                    TooltipPosition::Bottom,
                ),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .into();
        }
        let entry = Entry::new("Search recipes", self.search.text())
            .on_input(Message::SearchInput)
            .left_section(Icon::Search.view(16.0))
            .width(Length::Fill);
        let line = row![
            tooltip(
                icon_button(Icon::GoPrevious).on_press(Message::ToggleSearch),
                "Close search",
                TooltipPosition::Bottom,
            ),
            entry,
            self.filter_button(),
            self.sort_button(),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        column![
            line,
            tag_filter(
                &self.tags,
                &self.tag_groups,
                &self.tag_query,
                "A recipe needs every tag of at least one group. Press + to start another group.",
                Message::TagGroups,
                Message::TagQuery,
            ),
        ]
        .spacing(8)
        .into()
    }

    pub(super) fn grid_page(&self, width: f32) -> Element<'_, Message> {
        let mut head = column![self.search_row()].spacing(8);
        if let Some(strip) = self.active_filters() {
            head = head.push(strip);
        }

        let body: Element<'_, Message> = if self.recipes.is_empty() {
            self.empty_state()
        } else if self.compact {
            self.compact_list()
        } else {
            self.masonry(width)
        };
        column![container(head).padding([12, 16]), body]
            .spacing(4)
            .height(Length::Fill)
            .into()
    }

    fn empty_state(&self) -> Element<'_, Message> {
        if self.search.query().is_some() || self.filter_count() > 0 {
            return empty(
                Icon::Search,
                "No matches",
                "Try different words or fewer filters.",
            );
        }
        match self.view {
            View::All => empty(
                Icon::Folder,
                "Nothing yet",
                "Add the first recipe with the +.",
            ),
            View::Calendar | View::Fridge => space().into(),
        }
    }

    /// The filters that are on, each a chip that removes itself when pressed.
    fn active_filters(&self) -> Option<Element<'_, Message>> {
        if self.filter_count() == 0 {
            return None;
        }
        let mut tag_line: Option<Element<'_, Message>> = None;
        let mut chips = row![
            IconToggle::new(Icon::Filter, true)
                .tooltip("Edit filters")
                .on_toggle(|_| Message::OpenFilter)
        ]
        .spacing(4)
        .align_y(Alignment::Center);
        for c in active_filters(
            super::filter::kind(),
            &self.filters,
            &|_| Vec::new(),
            Message::Fields,
        ) {
            chips = chips.push(c);
        }
        // The tag groups: alternatives, set apart by a gap.
        if tag_count(&self.tag_groups) > 0 {
            let groups: Vec<Vec<String>> = self.tag_groups.groups().to_vec();
            let named = groups.clone();
            tag_line = Some(tag_groups(&groups, move |g, t| {
                Message::TagGroups(GroupEvent::Toggle(g, named[g][t].clone()))
            }));
        }
        // The tag groups read like the filter's input: on a line of their own, under the chips.
        Some(match tag_line {
            Some(t) => column![chips.wrap(), t].spacing(6).into(),
            None => chips.wrap().into(),
        })
    }

    /// One line per entry: the heart, the title, and the rating and list.
    fn compact_list(&self) -> Element<'_, Message> {
        let rows: Vec<Element<'_, Message>> =
            self.recipes.iter().map(|e| self.compact_row(e)).collect();
        scrolled_window(container(boxed_list(rows)).padding([4, 16]))
            .height(Length::Fill)
            .into()
    }

    fn compact_row<'a>(&'a self, e: &'a Recipe) -> Element<'a, Message> {
        let selected = self.details.as_ref().is_some_and(|d| d.entry.id == e.id);
        let mut line = row![text(ellipsize(&e.title, 80)).width(Length::Fill)]
            .spacing(8)
            .align_y(Alignment::Center);
        if let Some(r) = e.rating {
            line = line.push(text(stars(r)).size(13));
        }
        // The heart sits at the right edge: filled for a favourite, an outline for one that is not.
        line = line.push(heart(e.favorite, Message::Fave(e.id)));
        selectable(line.padding([2, 12]), selected, Message::Open(e.id))
    }

    /// Cards dealt across as many columns as fit, left to right, so a short card never leaves a gap.
    fn masonry(&self, width: f32) -> Element<'_, Message> {
        let n = (((width - 32.0) / CARD_WIDTH).floor() as usize).max(1);
        let mut cols: Vec<Vec<Element<'_, Message>>> = (0..n).map(|_| Vec::new()).collect();
        for (i, e) in self.recipes.iter().enumerate() {
            cols[i % n].push(self.card(e));
        }
        let grid = row(cols
            .into_iter()
            .map(|c| column(c).spacing(12).width(Length::FillPortion(1)).into()))
        .spacing(12);
        scrolled_window(container(grid).padding([4, 16]))
            .height(Length::Fill)
            .into()
    }

    fn card<'a>(&'a self, e: &'a Recipe) -> Element<'a, Message> {
        let selected = self.details.as_ref().is_some_and(|d| d.entry.id == e.id);
        // A card takes the colour of its first tag, so kinds of recipe are told apart at a glance.
        let tint = e.tags.first().map(|t| color_of(t));

        // The title, with the heart at the right edge: filled for a favourite, an outline if not.
        let mut body = column![].spacing(6);
        if let Some(first) = e.photos.first() {
            body = body.push(super::parts::photo(first, 150.0));
        }
        body = body.push(
            row![
                text(e.title.clone()).size(15).width(Length::Fill),
                heart(e.favorite, Message::Fave(e.id))
            ]
            .align_y(Alignment::Start),
        );
        if let Some(r) = e.rating {
            body = body.push(text(stars(r)).size(14));
        }
        let notes = snippet(&e.notes, 4, 180);
        if !notes.is_empty() {
            body = body.push(text(notes).size(13));
        }
        let tags = collapse(&e.tags, 3);
        if !tags.shown.is_empty() {
            let mut chips = row(tags.shown.iter().map(|g| hashtag(g))).spacing(4);
            if tags.more > 0 {
                chips = chips.push(more(tags.more));
            }
            body = body.push(chips.wrap());
        }

        card(body, tint, selected, Message::Open(e.id))
    }
}
