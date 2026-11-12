//! The kanban board: a column per status, each a stack of note cards in the style of Google
//! Keep. A card shows only the task's title, its tags as `# tags`, how far its subtasks are, and
//! where it belongs: its epic and section, or on an epic's own board just the section. Drag a
//! card to another column to change its status; pressing it opens its details.

use iced::widget::{column, row};
use iced::{Alignment, Length};
use organiser_ui::Element;
use organiser_ui::components::card::card;
use organiser_ui::components::chip::{hashtag, more};
use organiser_ui::components::dot::{color_from_hex, dot};
use organiser_ui::components::icons;
use organiser_ui::iad::prelude::*;
use organiser_ui::primitives::groups::group_by;
use organiser_ui::primitives::tags::collapse;
use organiser_ui::primitives::text::ellipsize;
use organiser_ui::widgets::board::{COLUMN_WIDTH, board_column};
use todo_schema::board::columns;
use todo_schema::task::Task;

use crate::app::{App, Message};

impl App {
    pub(super) fn board_page(&self, width: f32) -> Element<'_, Message> {
        let cols = columns(&self.board, self.board_view(), &self.statuses);
        // Columns share the width when they fit, and scroll sideways when they do not.
        let fits = width >= cols.len() as f32 * COLUMN_WIDTH + 48.0;
        let column_width = if fits {
            Length::FillPortion(1)
        } else {
            Length::Fixed(COLUMN_WIDTH)
        };
        let board = row(cols.into_iter().map(|(status, tasks)| {
            let count = tasks.len();
            // Pinned cards first, under their own heading (the tasks are already pinned first).
            let (pinned, others): (Vec<&Task>, Vec<&Task>) =
                tasks.into_iter().partition(|t| t.pinned);
            let mut cards: Vec<Element<'_, Message>> = Vec::new();
            if !pinned.is_empty() {
                cards.push(caption("Pinned").into());
                cards.extend(pinned.iter().map(|t| self.card(t)));
            }
            if self.grouped_by_epic() {
                // Then a group for each epic, and the tasks in none last, "Unassigned".
                let order: Vec<i64> = self.epics.iter().map(|s| s.epic.id).collect();
                for (epic, tasks) in group_by(others, |t| t.epic, &order) {
                    cards.push(self.epic_group_header(epic, None));
                    cards.extend(tasks.into_iter().map(|t| self.card(t)));
                }
            } else if !others.is_empty() {
                if !pinned.is_empty() {
                    cards.push(caption("Others").into());
                }
                cards.extend(others.into_iter().map(|t| self.card(t)));
            }
            let accent = status.color.as_deref().and_then(color_from_hex);
            drop_zone(
                board_column(&status.name, accent, count, cards, column_width),
                self.dragging.is_some(),
                Message::DropOn(status.id),
            )
            .into()
        }))
        .spacing(16)
        .height(Length::Fill);

        let body: Element<'_, Message> = if fits {
            board.into()
        } else {
            scrolled_window_horizontal(board)
                .height(Length::Fill)
                .into()
        };
        let body: Element<'_, Message> = match self.all_page_menu() {
            Some(menu) => column![container(menu).padding([0, 4]), body]
                .spacing(8)
                .height(Length::Fill)
                .into(),
            None => body,
        };
        container(body)
            .padding([12, 16])
            .height(Length::Fill)
            .into()
    }

    fn card<'a>(&'a self, t: &'a Task) -> Element<'a, Message> {
        let selected = self.details.as_ref().is_some_and(|d| d.id() == t.id);
        // The card takes its status's colour, as a Keep note takes its colour.
        let tint = self
            .statuses
            .resolve(t.status)
            .and_then(|s| s.color.as_deref())
            .and_then(color_from_hex);

        // No status dot: the column is the status.
        let mut body = column![text(t.title.clone()).size(15)].spacing(6);
        if let Some(parent) = t.parent_id.and_then(|id| self.task_titles.get(&id)) {
            body = body.push(caption(format!("in {}", ellipsize(parent, 40))));
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

        let card = card(body, tint, selected, Message::Select(t.id));
        drag_source(card, Message::DragStart(t.id)).into()
    }

    /// A task's chips: the icon-only sticky chip first, if it is a sticky, then its tags as
    /// `# tags` (the first `shown` of them and how many more). `None` when it has neither.
    pub(super) fn tag_row<'a>(&'a self, t: &'a Task, shown: usize) -> Option<Element<'a, Message>> {
        let tags = collapse(&t.tags, shown);
        let sticky = self.flags_row(t);
        if tags.shown.is_empty() && sticky.is_none() {
            return None;
        }
        let mut chips = row![].spacing(4).align_y(Alignment::Center);
        if let Some(s) = sticky {
            chips = chips.push(s);
        }
        for g in &tags.shown {
            chips = chips.push(hashtag(g));
        }
        if tags.more > 0 {
            chips = chips.push(more(tags.more));
        }
        Some(chips.wrap().into())
    }

    /// Where a task belongs, as chips: its epic and section, or on an epic's own page just the
    /// section. `None` when it belongs nowhere.
    pub(super) fn placement_row<'a>(&'a self, t: &'a Task) -> Option<Element<'a, Message>> {
        let mut place = row![].spacing(8).align_y(Alignment::Center);
        let mut any = false;
        if self.epic_filter.is_none()
            && !self.grouped_by_epic()
            && let Some(id) = t.epic
            && let Some(name) = self.epic_titles.get(&id)
        {
            place = place.push(icons::folder_chip(name, self.epic_color(id), self.epic_icon(id)));
            any = true;
        }
        if let Some(chip) = self.section_chip(t) {
            place = place.push(chip);
            any = true;
        }
        any.then(|| place.wrap().into())
    }

    /// The icon-only "sticky" chip of a task that is on the desktop as a sticky, for the start
    /// of its chips. `None` when it is not. (Pinned tasks are not marked: they are in the
    /// pinned section, or at the top of their list.)
    pub(super) fn flags_row<'a>(&'a self, t: &'a Task) -> Option<Element<'a, Message>> {
        t.sticky.then(|| {
            tooltip(
                Chip::from_icon(Icon::Note)
                    .role(Role::Warning)
                    .size(ChipSize::Small),
                "Sticky on the desktop",
                TooltipPosition::Bottom,
            )
            .into()
        })
    }

    /// A task's status as a dot in its colour; `None` when its status has no colour.
    pub(super) fn status_dot<'a>(&'a self, t: &'a Task) -> Option<Element<'a, Message>> {
        self.statuses
            .resolve(t.status)
            .and_then(|s| s.color.as_deref())
            .and_then(color_from_hex)
            .map(|c| dot(c, 9.0))
    }

    /// A card's title with the status dot at its right edge. The title wraps within the room the
    /// dot leaves, so a long one never runs under it.
    pub(super) fn titled<'a>(
        &'a self,
        lead: Option<Element<'a, Message>>,
        t: &'a Task,
    ) -> Element<'a, Message> {
        let mut line = row![].spacing(6).align_y(Alignment::Start);
        if let Some(l) = lead {
            line = line.push(l);
        }
        line = line.push(text(t.title.clone()).size(15).width(Length::Fill));
        if let Some(d) = self.status_dot(t) {
            // Level with the first line of the title, not the middle of a wrapped one.
            line = line.push(container(d).padding(iced::Padding::default().top(6.0)));
        }
        line.into()
    }

    /// The section a task is in, as a chip in its colour; `None` when it is in none.
    pub(super) fn section_chip<'a>(&'a self, t: &'a Task) -> Option<Element<'a, Message>> {
        let epic = t.epic?;
        let sec = self
            .epic_sections
            .get(&epic)
            .and_then(|v| v.iter().find(|s| Some(s.section.id) == t.section))?;
        let chip = Chip::new(ellipsize(&sec.section.title, 24)).size(ChipSize::Small);
        Some(match sec.section.color.as_deref().and_then(color_from_hex) {
            Some(c) => chip.color(c).into(),
            None => chip.role(Role::Neutral).into(),
        })
    }
}
