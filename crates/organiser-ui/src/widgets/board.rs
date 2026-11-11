//! One column of a kanban board: a heading with a count over a scrolling stack of cards. A column
//! with an `accent` colour gets a dot beside its heading and a faint wash of that colour behind
//! it, so the columns read apart at a glance.

use iced::widget::{column, row};
use iced::{Alignment, Background, Border, Color, Length};

use crate::Element;
use crate::components::dot::dot;
use crate::iad::Theme;
use crate::iad::prelude::*;

/// Minimum width of a column, so a narrow window scrolls the board sideways rather than squeezing it.
pub const COLUMN_WIDTH: f32 = 260.0;

/// How strong a column's wash is: just enough to see where it is.
const WASH: f32 = 0.07;

/// `width` is `FillPortion(1)` to share the board equally, or `Fixed(COLUMN_WIDTH)` to scroll.
pub fn board_column<'a, M: Clone + 'a>(
    title: &str,
    accent: Option<Color>,
    cards: Vec<Element<'a, M>>,
    width: Length,
) -> Element<'a, M> {
    let count = cards.len();
    let mut head = row![].spacing(8).align_y(Alignment::Center);
    if let Some(c) = accent {
        head = head.push(dot(c, 10.0));
    }
    let head = head
        .push(heading(title.to_string()))
        .push(caption(count.to_string()));
    let body: Element<'a, M> = if cards.is_empty() {
        caption("Nothing here").into()
    } else {
        column(cards).spacing(10).width(Length::Fill).into()
    };
    let inner = column![
        head,
        scrolled_window(container(body).padding([0, 8])).height(Length::Fill)
    ]
    .spacing(10)
    .width(Length::Fill)
    .height(Length::Fill);
    container(inner)
        .padding(10)
        .width(width)
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: accent.map(|c| Background::Color(Color { a: WASH, ..c })),
            border: Border::default().rounded(12.0),
            ..container::Style::default()
        })
        .into()
}
