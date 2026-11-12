//! The notes view: every task a note, in a grid of cards laid out like Google Keep (the layout
//! is shared with the Notes app, see `organiser_ui::widgets::masonry`). A card shows the title,
//! the details as they read (clipped), how far its subtasks are, its tags, where it belongs and
//! its due date. Pressing a card opens its details. A note takes its epic's colour.

use iced::widget::column;
use iced::Length;
use organiser_ui::Element;
use organiser_ui::components::card::card;
use organiser_ui::components::chip::pill;
use organiser_ui::components::empty::empty;
use organiser_ui::iad::prelude::*;
use organiser_ui::primitives::groups::group_by;
use organiser_ui::widgets::masonry::{PAGE_PADDING, columns_for, masonry};
use todo_schema::due::{Bucket, bucket, label};
use todo_schema::task::Task;

use crate::app::{App, Message};

/// The tallest the drawn details of a card get.
const CARD_BODY: f32 = 170.0;

/// A rough height for packing: a title line, the details' lines, and a row each for what follows.
fn weight(t: &Task, docs: bool) -> usize {
    2 + if docs {
        t.notes.lines().filter(|l| !l.trim().is_empty()).count().min(6)
    } else {
        0
    } + usize::from(!t.tags.is_empty())
        + usize::from(t.epic.is_some())
}

impl App {
    /// `room` is the width the page has.
    pub(super) fn notes_page<'a>(&'a self, room: f32) -> Element<'a, Message> {
        let mut page = column![].spacing(16).width(Length::Fill);
        if let Some(epic) = self.epic_filter.filter(|_| self.scope == todo_schema::views::Scope::All)
        {
            page = page.push(self.epic_header(epic));
        }
        if let Some(menu) = self.all_page_menu() {
            page = page.push(menu);
        }
        if let Some(strip) = self.active_filters() {
            page = page.push(strip);
        }
        // Finished tasks are notes too, but only when asked for.
        let shown: Vec<&Task> = self
            .board
            .iter()
            .filter(|t| self.show_closed || self.statuses.is_open(t.status))
            .collect();
        if shown.is_empty() {
            let searching = self.search.query().is_some();
            let (title, text) = if searching || self.filter_count() > 0 {
                ("No matching tasks", "Try a different search or filter.")
            } else {
                ("Nothing to do", "Press + to add a task.")
            };
            page = page.push(empty(Icon::Check, title, text));
        } else {
            let cols = columns_for(room);
            let (pinned, others): (Vec<&Task>, Vec<&Task>) =
                shown.into_iter().partition(|t| t.pinned);
            let grid = |tasks: Vec<&'a Task>| {
                masonry(
                    tasks
                        .into_iter()
                        .map(|t| (weight(t, self.docs.contains_key(&t.id)), self.note(t))),
                    cols,
                )
            };
            // Pinned notes come first, under their own heading.
            let any_pinned = !pinned.is_empty();
            if any_pinned {
                page = page.push(caption("Pinned")).push(grid(pinned));
            }
            if self.grouped_by_epic() {
                // Then a group for each epic, and the tasks in none last, "Unassigned".
                let order: Vec<i64> = self.epics.iter().map(|s| s.epic.id).collect();
                for (epic, tasks) in group_by(others, |t| t.epic, &order) {
                    page = page.push(self.epic_group_header(epic, Some(tasks.len())));
                    page = page.push(grid(tasks));
                }
            } else if !others.is_empty() {
                if any_pinned {
                    page = page.push(caption("Others"));
                }
                page = page.push(grid(others));
            }
        }
        scrolled_window(container(page).padding(PAGE_PADDING))
            .height(Length::Fill)
            .into()
    }

    fn note<'a>(&'a self, t: &'a Task) -> Element<'a, Message> {
        let selected = self.details.as_ref().is_some_and(|d| d.id() == t.id);
        let closed = self.statuses.is_closed(t.status);
        let id = t.id;
        // The note takes its epic's colour, as a Keep note takes its colour.
        let tint = t.epic.and_then(|e| self.epic_color(e));

        let check: Element<'_, Message> = check_button("", closed)
            .on_toggle(move |_| Message::Toggle(id))
            .into();
        let mut body = column![self.titled(Some(check), t)].spacing(6);
        if let Some(doc) = self.docs.get(&t.id) {
            // The details as they read, clipped: a card is a preview, not the whole task.
            body = body.push(
                container(RichText::new(&self.theme(), doc).compact(true).max_blocks(6))
                    .max_height(CARD_BODY)
                    .clip(true),
            );
        }
        if let Some((done, total)) = self.sub_progress.get(&t.id) {
            body = body.push(caption(format!("✓ {done} of {total} subtasks done")));
        }
        if let Some(tags) = self.tag_row(t, 3) {
            body = body.push(tags);
        }
        if let Some(place) = self.placement_row(t) {
            body = body.push(place);
        }
        // The date is always last.
        if let Some(d) = t.due {
            let role = match (closed, bucket(d, self.today)) {
                (true, _) => Role::Success,
                (_, Bucket::Overdue) => Role::Error,
                (_, Bucket::Today) => Role::Warning,
                _ => Role::Accent,
            };
            body = body.push(pill(&label(d, self.today), role));
        }
        card(body, tint, selected, Message::Select(t.id))
    }
}
