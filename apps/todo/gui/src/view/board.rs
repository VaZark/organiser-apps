//! The kanban board: a column per status, each a stack of note cards in the style of Google
//! Keep. A card shows the task's title, a snippet of its details, how far its subtasks are, its
//! tags as `# tags`, and what matters at a glance: priority, epic and due date. The ◀ ▶ buttons move
//! it to the next column; pressing the card opens its details.

use iced::widget::{column, row};
use iced::{Alignment, Length};
use organiser_ui::Element;
use organiser_ui::components::card::card;
use organiser_ui::components::chip::{color_of, hashtag, more, pill};
use organiser_ui::iad::prelude::*;
use organiser_ui::primitives::tags::collapse;
use organiser_ui::primitives::text::{ellipsize, snippet};
use organiser_ui::widgets::board::{COLUMN_WIDTH, board_column};
use todo_schema::board::columns;
use todo_schema::due::{Bucket, bucket, label};
use todo_schema::priority::Priority;
use todo_schema::task::Task;

use crate::app::{App, Message};

impl App {
    pub(super) fn board_page(&self, width: f32) -> Element<'_, Message> {
        let placeholder = match self.epic_filter.and_then(|id| self.epic_titles.get(&id)) {
            Some(e) => format!("Add a task to {e}…  #tag  due:tomorrow"),
            None => "Add a task…  #tag  due:tomorrow".to_string(),
        };
        let quick = entry(placeholder, self.quick.as_str())
            .on_input(Message::QuickText)
            .on_submit(Message::QuickSubmit)
            .left_section(Icon::ListAdd.view(16.0));

        let cols = columns(&self.board, self.board_view());
        // Columns share the width when they fit, and scroll sideways when they do not.
        let fits = width >= cols.len() as f32 * COLUMN_WIDTH + 48.0;
        let column_width = if fits {
            Length::FillPortion(1)
        } else {
            Length::Fixed(COLUMN_WIDTH)
        };
        let board = row(cols.into_iter().map(|(status, tasks)| {
            let cards = tasks.into_iter().map(|t| self.card(t)).collect();
            board_column(status.label(), cards, column_width)
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
        column![
            container(quick).padding([12, 16]),
            container(body).padding([0, 16]).height(Length::Fill)
        ]
        .spacing(4)
        .height(Length::Fill)
        .into()
    }

    fn card<'a>(&'a self, t: &'a Task) -> Element<'a, Message> {
        let selected = self.details.as_ref().is_some_and(|d| d.id() == t.id);
        let epic = t.epic.and_then(|id| self.epic_titles.get(&id));
        // The card takes its epic's colour, as a Keep note takes its colour.
        let tint = epic.map(|e| color_of(e));

        let mut body = column![text(t.title.clone()).size(15)].spacing(6);
        if let Some(parent) = t.parent_id.and_then(|id| self.task_titles.get(&id)) {
            body = body.push(caption(format!("in {}", ellipsize(parent, 40))));
        }
        let notes = snippet(&t.notes, 3, 140);
        if !notes.is_empty() {
            body = body.push(text(notes).size(13));
        }
        if let Some((done, total)) = self.sub_progress.get(&t.id) {
            body = body.push(caption(format!("✓ {done} of {total} subtasks done")));
        }

        let tags = collapse(&t.tags, 3);
        if !tags.shown.is_empty() {
            let mut chips = row(tags.shown.iter().map(|g| hashtag(g))).spacing(4);
            if tags.more > 0 {
                chips = chips.push(more(tags.more));
            }
            body = body.push(chips.wrap());
        }

        // What matters at a glance, and the buttons that move the card.
        let mut facts = row![].spacing(4).align_y(Alignment::Center);
        if t.priority.is_urgent() {
            let role = if t.priority == Priority::Highest {
                Role::Error
            } else {
                Role::Warning
            };
            facts = facts.push(pill(t.priority.label(), role));
        }
        if let Some(e) = epic {
            facts = facts.push(pill(e, Role::Warning));
        }
        if let Some(d) = t.due {
            let role = match (t.status.is_closed(), bucket(d, self.today)) {
                (true, _) => Role::Success,
                (_, Bucket::Overdue) => Role::Error,
                (_, Bucket::Today) => Role::Warning,
                _ => Role::Accent,
            };
            facts = facts.push(pill(&label(d, self.today), role));
        }
        let mut moves = row![].spacing(2);
        if self.board_view().neighbor(t.status, -1).is_some() {
            moves = moves.push(icon_button(Icon::GoPrevious).on_press(Message::MoveCard(t.id, -1)));
        }
        if self.board_view().neighbor(t.status, 1).is_some() {
            moves = moves.push(icon_button(Icon::GoNext).on_press(Message::MoveCard(t.id, 1)));
        }
        body = body.push(
            row![container(facts.wrap()).width(Length::Fill), moves].align_y(Alignment::Center),
        );

        card(body, tint, selected, Message::Select(t.id))
    }
}
