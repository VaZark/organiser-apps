//! The task list: quick add and the nested tasks.

use iced::Length;
use iced::widget::{column, row};
use organiser_engine::Row;
use organiser_ui::Element;
use organiser_ui::components::chip::{chip, more, pill};
use organiser_ui::components::empty::empty;
use organiser_ui::components::row::selectable;
use organiser_ui::iad::prelude::*;
use organiser_ui::primitives::expand::plain_rows;
use organiser_ui::primitives::tags::collapse;
use organiser_ui::primitives::text::ellipsize;
use todo_schema::due::{Bucket, bucket, label};
use todo_schema::priority::Priority;
use todo_schema::status::Status;
use todo_schema::task::Task;

use crate::app::{App, Message};

const INDENT: f32 = 24.0;
/// The most width the epic, date and tags take before they wrap.
const SIDE_WIDTH: f32 = 270.0;

impl App {
    /// `stacked`: the list is too narrow for chips beside the titles.
    pub(super) fn list_page(&self, stacked: bool) -> Element<'_, Message> {
        let placeholder = match self.epic_filter.and_then(|id| self.epic_titles.get(&id)) {
            Some(e) => format!("Add a task to {e}…  #tag  due:tomorrow"),
            None => "Add a task…  #tag  due:tomorrow".to_string(),
        };
        let quick = entry(placeholder, self.quick.as_str())
            .on_input(Message::QuickText)
            .on_submit(Message::QuickSubmit)
            .left_section(Icon::ListAdd.view(16.0));

        let rows = if self.is_flat() {
            plain_rows(&self.flat_tasks)
        } else {
            self.folds.rows(&self.forest, |t| t.id)
        };
        let body: Element<'_, Message> = if rows.is_empty() {
            let searching = self.search.query().is_some();
            let (title, text) = if searching || self.filter_count() > 0 {
                ("No matching tasks", "Try a different search or filter.")
            } else {
                ("Nothing to do", "Add a task above to get started.")
            };
            empty(Icon::Check, title, text)
        } else {
            boxed_list(rows.iter().map(|r| self.task_row(r, stacked)))
        };
        let mut col = column![quick].spacing(12).width(Length::Fill);
        if let Some(strip) = self.active_filters() {
            col = col.push(strip);
        }
        col.push(body).into()
    }

    /// The filters that are on, each a chip that removes itself when pressed.
    fn active_filters(&self) -> Option<Element<'_, Message>> {
        if self.filter_count() == 0 {
            return None;
        }
        let removable = |label: String, on_press: Message| -> Element<'_, Message> {
            flat_button(
                row![text(label), Icon::WindowClose.view(12.0)]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
            )
            .on_press(on_press)
            .into()
        };
        let mut chips = row![icon_button(Icon::Filter).on_press(Message::OpenFilter)]
            .spacing(4)
            .align_y(iced::Alignment::Center);
        if let Some(id) = self.epic_filter {
            let name = self
                .epic_titles
                .get(&id)
                .cloned()
                .unwrap_or_else(|| "Epic".into());
            chips = chips.push(removable(name, Message::ToggleEpic(id)));
        }
        for t in &self.tag_filters {
            chips = chips.push(removable(t.clone(), Message::ToggleTag(t.clone())));
        }
        Some(chips.wrap().into())
    }

    fn task_row<'a>(&'a self, r: &Row<'a, Task>, stacked: bool) -> Element<'a, Message> {
        let t = r.value;
        let closed = t.status.is_closed();
        let selected = self.details.as_ref().is_some_and(|d| d.id() == t.id);
        let fold: Element<'_, Message> = if r.has_children {
            let icon = if r.expanded {
                Icon::GoDown
            } else {
                Icon::GoNext
            };
            icon_button(icon).on_press(Message::Fold(t.id)).into()
        } else if self.is_flat() {
            // Nothing is nested, so there is nothing to line up with.
            space().width(Length::Fixed(0.0)).into()
        } else {
            space().width(Length::Fixed(32.0)).into()
        };
        let check =
            check_button("", t.status == Status::Done).on_toggle(move |_| Message::Toggle(t.id));

        // The title, and under it what is not obvious from the checkbox: in progress, won't do,
        // a high priority.
        let mut note = row![].spacing(4);
        let mut has_note = false;
        let mut push = |e: Element<'a, Message>| {
            note = std::mem::replace(&mut note, row![]).push(e);
            has_note = true;
        };
        if t.status == Status::InProgress {
            push(pill("In progress", Role::Accent));
        }
        if t.status == Status::Dismissed {
            push(pill(t.status.label(), Role::Neutral));
        }
        if t.priority.is_urgent() {
            let role = if t.priority == Priority::Highest {
                Role::Error
            } else {
                Role::Warning
            };
            push(pill(t.priority.label(), role));
        }
        let mut title = column![text(t.title.clone())].spacing(2);
        // Without nesting a subtask would lose its context, so say whose it is.
        if self.is_flat()
            && let Some(parent) = t.parent_id.and_then(|id| self.task_titles.get(&id))
        {
            title = title.push(caption(format!("in {}", ellipsize(parent, 40))));
        }
        if has_note {
            title = title.push(note);
        }
        // A row is one button (see `selectable`), so the title is just text that fills the line.
        let title = title.width(Length::Fill).padding([6, 8]);

        // On the side, always when set: the epic, the due date, then the person's own tags.
        let mut side = row![].spacing(4);
        if let Some(e) = t.epic.and_then(|id| self.epic_titles.get(&id)) {
            side = side.push(pill(e, Role::Warning));
        }
        if let Some(d) = t.due {
            let role = match (closed, bucket(d, self.today)) {
                (true, _) => Role::Success,
                (_, Bucket::Overdue) => Role::Error,
                (_, Bucket::Today) => Role::Warning,
                _ => Role::Accent,
            };
            side = side.push(pill(&label(d, self.today), role));
        }
        let tags = collapse(&t.tags, 2);
        for g in &tags.shown {
            side = side.push(chip(g));
        }
        if tags.more > 0 {
            side = side.push(more(tags.more));
        }

        // Beside the title when there is room for both. Otherwise the chips go under it and wrap
        // there, so the title keeps the full width instead of breaking one word per line.
        let main: Element<'a, Message> = if stacked {
            column![title, side.wrap()]
                .spacing(4)
                .width(Length::Fill)
                .into()
        } else {
            row![title, container(side.wrap()).max_width(SIDE_WIDTH)]
                .spacing(6)
                .align_y(iced::Alignment::Center)
                .width(Length::Fill)
                .into()
        };
        let line = row![
            space().width(Length::Fixed(INDENT * r.depth as f32)),
            fold,
            check,
            main,
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center)
        // Room at both edges in every mode: in a flat list there is no fold arrow to space the
        // checkbox from the edge, and the chips must not touch the right one.
        .padding([4, 12]);
        selectable(line, selected, Message::Select(t.id))
    }
}
