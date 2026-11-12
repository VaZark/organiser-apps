//! The quadrants: the Eisenhower matrix as four boxes, urgent down the left and important across
//! the top, each a compact list of the tasks that belong there (see `todo_schema::quadrant`). A
//! task is in a box by its priority and due date alone, and each box is in triage order: the more
//! important first, then the closer date.
//! Pressing a row opens the task; the checkbox finishes it. In a narrow window the boxes stack.

use iced::widget::{column, row};
use iced::{Alignment, Length};
use organiser_ui::Element;
use organiser_ui::components::chip::pill;
use organiser_ui::components::row::selectable;
use organiser_ui::iad::prelude::*;
use organiser_ui::iad::styles::container::card;
use todo_schema::due::{Bucket, bucket, label};
use todo_schema::quadrant::{Quadrant, classify, triage_order};
use todo_schema::task::Task;
use todo_schema::views::Scope;

use crate::app::{App, Message};

/// Below this width the four boxes stack instead of making a grid.
const GRID_FROM: f32 = 720.0;

impl App {
    /// `room` is the width the page has.
    pub(super) fn quadrants_page(&self, room: f32) -> Element<'_, Message> {
        let mut page = column![].spacing(12).width(Length::Fill);
        if let Some(epic) = self.epic_filter.filter(|_| self.scope == Scope::All) {
            page = page.push(self.epic_header(epic));
        }
        if let Some(strip) = self.active_filters() {
            page = page.push(strip);
        }
        // A task is in a box by its priority and due date alone, whatever its status. Inside a
        // box, triage order: the more important first, then the closer date.
        let subtasks: std::collections::HashMap<i64, usize> = self
            .sub_progress
            .iter()
            .map(|(id, (_, n))| (*id, *n))
            .collect();
        let tasks_in = |q: Quadrant| -> Vec<&Task> {
            let mut v: Vec<&Task> = self
                .board
                .iter()
                .filter(|t| classify(t.priority, t.due, self.today) == q)
                .collect();
            v.sort_by(|a, b| triage_order(a, b, &subtasks));
            v
        };
        let grid = room >= GRID_FROM;
        let [a, b, c, d] = Quadrant::ALL.map(|q| self.quadrant(q, &tasks_in(q), grid));
        let body: Element<'_, Message> = if grid {
            column![
                row![a, b].spacing(12).height(Length::FillPortion(1)),
                row![c, d].spacing(12).height(Length::FillPortion(1)),
            ]
            .spacing(12)
            .height(Length::Fill)
            .into()
        } else {
            column![a, b, c, d].spacing(12).into()
        };
        page = page.push(body);
        if grid {
            // The page itself does not scroll: each box does.
            container(page.height(Length::Fill)).padding(16).into()
        } else {
            scrolled_window(container(page).padding(16))
                .height(Length::Fill)
                .into()
        }
    }

    /// One box: its name and what puts a task in it, over a compact list.
    fn quadrant<'a>(&'a self, q: Quadrant, tasks: &[&'a Task], grid: bool) -> Element<'a, Message> {
        // What puts a task here, as chips: urgent, important, both or neither.
        let mut head = row![text(q.title()).size(15)]
            .spacing(8)
            .align_y(Alignment::Center);
        if q.urgent() {
            head = head.push(pill("Urgent", Role::Error));
        }
        if q.important() {
            head = head.push(pill("Important", Role::Warning));
        }
        let head = row![
            head,
            container(caption(tasks.len().to_string()))
                .width(Length::Fill)
                .align_right(Length::Fill),
        ]
        .align_y(Alignment::Center);
        let list: Element<'a, Message> = if tasks.is_empty() {
            caption("Nothing here").into()
        } else {
            // Pinned tasks first, under their own heading, as in the list and the board (the
            // triage order already has them on top).
            let (pinned, others): (Vec<&Task>, Vec<&Task>) =
                tasks.iter().copied().partition(|t| t.pinned);
            let mut rows = column![].spacing(2);
            if !pinned.is_empty() {
                rows = rows.push(caption("Pinned"));
                rows = rows.extend(pinned.into_iter().map(|t| self.compact_row(t)));
                if !others.is_empty() {
                    rows = rows.push(caption("Others"));
                }
            }
            rows.extend(others.into_iter().map(|t| self.compact_row(t))).into()
        };
        let body = column![head, if grid { scrolled_window(list).height(Length::Fill).into() } else { list }]
            .spacing(8)
            .width(Length::Fill);
        container(body)
            .padding(12)
            .width(Length::FillPortion(1))
            .height(if grid { Length::Fill } else { Length::Shrink })
            .style(card)
            .into()
    }

    /// A task as one line: a checkbox, its title, its status as a dot, and, last, its due date.
    fn compact_row<'a>(&'a self, t: &'a Task) -> Element<'a, Message> {
        let id = t.id;
        let closed = self.statuses.is_closed(t.status);
        let selected = self.details.as_ref().is_some_and(|d| d.id() == id);
        let mut line = row![
            check_button("", closed).on_toggle(move |_| Message::Toggle(id)),
            text(t.title.clone()).size(13).width(Length::Fill),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        // The task's status as a dot in its colour, before the chips.
        if let Some(d) = self.status_dot(t) {
            line = line.push(d);
        }
        if let Some(d) = t.due {
            let role = match (closed, bucket(d, self.today)) {
                (true, _) => Role::Success,
                (_, Bucket::Overdue) => Role::Error,
                (_, Bucket::Today) => Role::Warning,
                _ => Role::Accent,
            };
            line = line.push(pill(&label(d, self.today), role));
        }
        selectable(container(line).padding([4, 8]), selected, Message::Select(id))
    }
}
