//! One column of a kanban board: a heading with a count over a scrolling stack of cards.

use iced::widget::{column, row};
use iced::{Alignment, Length};

use crate::Element;
use crate::iad::prelude::*;

/// Minimum width of a column, so a narrow window scrolls the board sideways rather than squeezing it.
pub const COLUMN_WIDTH: f32 = 260.0;

/// `width` is `FillPortion(1)` to share the board equally, or `Fixed(COLUMN_WIDTH)` to scroll.
pub fn board_column<'a, M: Clone + 'a>(
    title: &str,
    cards: Vec<Element<'a, M>>,
    width: Length,
) -> Element<'a, M> {
    let count = cards.len();
    let head = row![heading(title.to_string()), caption(count.to_string())]
        .spacing(8)
        .align_y(Alignment::Center);
    let body: Element<'a, M> = if cards.is_empty() {
        caption("Nothing here").into()
    } else {
        column(cards).spacing(10).width(Length::Fill).into()
    };
    column![
        head,
        scrolled_window(container(body).padding([0, 8])).height(Length::Fill)
    ]
    .spacing(10)
    .width(width)
    .height(Length::Fill)
    .into()
}
