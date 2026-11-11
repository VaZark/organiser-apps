//! The kanban board: a column per status, each a stack of note cards in the style of Google
//! Keep. A card shows the task's title, a snippet of its details, how far its subtasks are, its
//! tags as `# tags`, and what matters at a glance: priority, epic and due date. The ◀ ▶ buttons move
//! it to the next column; pressing the card opens its details.

use iced::widget::{column, row};
use iced::{Alignment, Length};
use organiser_ui::Element;
use organiser_ui::components::card::card;
use organiser_ui::components::chip::{hashtag, more, pill};
use organiser_ui::components::dot::color_from_hex;
use organiser_ui::iad::prelude::*;
use organiser_ui::primitives::tags::collapse;
use organiser_ui::primitives::text::ellipsize;
use organiser_ui::widgets::board::{COLUMN_WIDTH, board_column};
use todo_schema::board::columns;
use todo_schema::due::{Bucket, bucket, label};
use todo_schema::priority::Priority;
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
            let cards = tasks.into_iter().map(|t| self.card(t)).collect();
            let accent = status.color.as_deref().and_then(color_from_hex);
            board_column(&status.name, accent, cards, column_width)
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
        container(body)
            .padding([12, 16])
            .height(Length::Fill)
            .into()
    }

    fn card<'a>(&'a self, t: &'a Task) -> Element<'a, Message> {
        let selected = self.details.as_ref().is_some_and(|d| d.id() == t.id);
        let epic = t.epic.and_then(|id| self.epic_titles.get(&id));
        // The card takes its status's colour, as a Keep note takes its colour.
        let status = self.statuses.resolve(t.status);
        let tint = status
            .and_then(|s| s.color.as_deref())
            .and_then(color_from_hex);
        let closed = status.is_some_and(|s| s.closed);

        let mut body = column![text(t.title.clone()).size(15)].spacing(6);
        if let Some(parent) = t.parent_id.and_then(|id| self.task_titles.get(&id)) {
            body = body.push(caption(format!("in {}", ellipsize(parent, 40))));
        }
        // The notes as they read, clipped: a card previews, it does not show everything.
        if let Some(doc) = self.docs.get(&t.id) {
            body = body.push(
                container(
                    RichText::new(&self.theme(), doc)
                        .compact(true)
                        .max_blocks(3),
                )
                .max_height(110.0)
                .clip(true),
            );
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
            let role = match (closed, bucket(d, self.today)) {
                (true, _) => Role::Success,
                (_, Bucket::Overdue) => Role::Error,
                (_, Bucket::Today) => Role::Warning,
                _ => Role::Accent,
            };
            facts = facts.push(pill(&label(d, self.today), role));
        }
        let mut moves = row![].spacing(2);
        let at = status.map(|s| s.id).unwrap_or_default();
        if self.board_view().neighbor(&self.statuses, at, -1).is_some() {
            moves = moves.push(icon_button(Icon::GoPrevious).on_press(Message::MoveCard(t.id, -1)));
        }
        if self.board_view().neighbor(&self.statuses, at, 1).is_some() {
            moves = moves.push(icon_button(Icon::GoNext).on_press(Message::MoveCard(t.id, 1)));
        }
        body = body.push(
            row![container(facts.wrap()).width(Length::Fill), moves].align_y(Alignment::Center),
        );

        card(body, tint, selected, Message::Select(t.id))
    }
}
