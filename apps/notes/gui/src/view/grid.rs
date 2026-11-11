//! The cards, laid out like Google Keep: as many columns as fit, each card in the shortest one
//! so tall and short notes pack together. Pinned notes come first under their own heading. A
//! card takes the note's own colour, whatever it is.

use iced::widget::{column, row};
use iced::{Alignment, Length};
use notes_schema::note::Note;
use notes_schema::views::Scope;
use organiser_ui::Element;
use organiser_ui::components::card::card;
use organiser_ui::components::chip::{hashtag, more};
use organiser_ui::components::empty::empty;
use organiser_ui::components::tag_groups::tag_groups;
use organiser_ui::components::tag_query::count as tag_count;
use organiser_ui::iad::Theme;
use organiser_ui::iad::prelude::*;
use organiser_ui::iad::primitives::tag_groups::GroupEvent;
use organiser_ui::primitives::tags::collapse;
use organiser_ui::widgets::schema_filters::active_filters;

use crate::app::{App, Message, paint};

/// The narrowest a card column gets, so cards stay readable.
const CARD_WIDTH: f32 = 240.0;
const GAP: f32 = 12.0;
/// The tallest the drawn body of a card gets.
const CARD_BODY: f32 = 170.0;
const PAGE_PADDING: f32 = 16.0;

/// How many card columns fit in `width`, at least one.
fn columns_for(width: f32) -> usize {
    let room = (width - 2.0 * PAGE_PADDING + GAP).max(CARD_WIDTH);
    ((room / (CARD_WIDTH + GAP)) as usize).max(1)
}

/// A rough height for packing: a title line plus the snippet's lines and a row for tags.
fn weight(n: &Note) -> usize {
    2 + n
        .body
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count()
        .min(6)
        + usize::from(!n.tags.is_empty())
}

impl App {
    pub(super) fn grid_page(&self, width: f32) -> Element<'_, Message> {
        let theme = self.theme();
        let mut page = column![].spacing(16).width(Length::Fill);
        if self.scope == Scope::Trash && !self.notes.is_empty() {
            page = page.push(
                row![
                    container(caption(format!(
                        "Notes in the trash are deleted after {} days.",
                        self.prefs.settings.notes.trash_days
                    )))
                    .width(Length::Fill),
                    destructive_button(text("Empty trash")).on_press(Message::EmptyTrash),
                ]
                .align_y(Alignment::Center),
            );
        }

        if let Some(strip) = self.active_filters() {
            page = page.push(strip);
        }

        if self.notes.is_empty() {
            let (icon, title, hint) = match (self.scope, self.search.query()) {
                (_, Some(_)) => (Icon::Search, "No notes found", "Try another word."),
                (Scope::Notes, _) => (Icon::Note, "No notes yet", "Notes you add appear here."),
                (Scope::Archive, _) => (
                    Icon::Archive,
                    "Nothing archived",
                    "Archived notes stay out of the way.",
                ),
                (Scope::Trash, _) => (
                    Icon::Delete,
                    "The trash is empty",
                    "Deleted notes wait here for a while.",
                ),
            };
            page = page.push(empty(icon, title, hint));
        } else {
            let cols = columns_for(width);
            let (pinned, others) = self.pinned();
            if !pinned.is_empty() {
                page = page
                    .push(caption("Pinned"))
                    .push(self.masonry(&theme, &pinned, cols));
                if !others.is_empty() {
                    page = page.push(caption("Others"));
                }
            }
            if !others.is_empty() {
                page = page.push(self.masonry(&theme, &others, cols));
            }
        }
        scrolled_window(container(page).padding(PAGE_PADDING))
            .height(Length::Fill)
            .into()
    }

    /// The filters that are on, each a chip that removes itself when pressed.
    fn active_filters(&self) -> Option<Element<'_, Message>> {
        if self.filter_count() == 0 {
            return None;
        }
        let mut tag_line: Option<Element<'_, Message>> = None;
        let mut chips = row![icon_button(Icon::Filter).on_press(Message::OpenFilter)]
            .spacing(4)
            .align_y(Alignment::Center);
        // The groups: alternatives, with a comma between them.
        if tag_count(&self.tag_groups) > 0 {
            let groups: Vec<Vec<String>> = self.tag_groups.groups().to_vec();
            let named = groups.clone();
            tag_line = Some(tag_groups(&groups, move |g, t| {
                Message::TagGroups(GroupEvent::Toggle(g, named[g][t].clone()))
            }));
        }
        for c in active_filters(
            super::filter::kind(),
            &self.filters,
            &|_| Vec::new(),
            Message::Fields,
        ) {
            chips = chips.push(c);
        }
        // The tag groups read like the filter's input: on a line of their own, under the chips.
        Some(match tag_line {
            Some(t) => column![chips.wrap(), t].spacing(6).into(),
            None => chips.wrap().into(),
        })
    }

    /// `notes` dealt into `cols` columns, each card to the lightest column so far.
    fn masonry<'a>(
        &'a self,
        theme: &Theme,
        notes: &[&'a Note],
        cols: usize,
    ) -> Element<'a, Message> {
        let mut lanes: Vec<(usize, Vec<Element<'a, Message>>)> =
            (0..cols).map(|_| (0, Vec::new())).collect();
        for n in notes {
            let lane = lanes
                .iter_mut()
                .min_by_key(|(load, _)| *load)
                .expect("at least one column");
            lane.0 += weight(n);
            lane.1.push(self.card(theme, n));
        }
        row(lanes.into_iter().map(|(_, cards)| {
            column(cards)
                .spacing(GAP)
                .width(Length::FillPortion(1))
                .into()
        }))
        .spacing(GAP)
        .align_y(Alignment::Start)
        .into()
    }

    fn card<'a>(&'a self, theme: &Theme, n: &'a Note) -> Element<'a, Message> {
        let selected = self.details.as_ref().is_some_and(|d| d.note.id == n.id);
        let mut body = column![text(n.title.clone()).size(15)].spacing(6);
        if let Some(doc) = self.docs.get(&n.id) {
            // The note as it reads, clipped: a card is a preview, not the whole note.
            body = body.push(
                container(RichText::new(theme, doc).compact(true).max_blocks(6))
                    .max_height(CARD_BODY)
                    .clip(true),
            );
        }
        let tags = collapse(&n.tags, 3);
        if !tags.shown.is_empty() {
            let mut chips = row(tags.shown.iter().map(|g| hashtag(g))).spacing(4);
            if tags.more > 0 {
                chips = chips.push(more(tags.more));
            }
            body = body.push(chips.wrap());
        }
        card(body, n.color.map(paint), selected, Message::Select(n.id))
    }
}
