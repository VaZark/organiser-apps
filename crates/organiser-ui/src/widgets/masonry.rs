//! Cards laid out like Google Keep: as many columns as fit, each card dealt to the lightest
//! column so far, so tall and short cards pack together. Shared by the apps that show their
//! items as notes.

use iced::widget::{column, row};
use iced::{Alignment, Length};

use crate::Element;

/// The narrowest a card column gets, so cards stay readable.
pub const CARD_WIDTH: f32 = 240.0;
/// The space between cards, across and down.
pub const GAP: f32 = 12.0;
/// The space around the whole grid.
pub const PAGE_PADDING: f32 = 16.0;

/// How many card columns fit in `width`, at least one.
pub fn columns_for(width: f32) -> usize {
    let room = (width - 2.0 * PAGE_PADDING + GAP).max(CARD_WIDTH);
    ((room / (CARD_WIDTH + GAP)) as usize).max(1)
}

/// `cards` (each with a rough height, for packing) dealt into `cols` columns.
pub fn masonry<'a, M: 'a>(
    cards: impl IntoIterator<Item = (usize, Element<'a, M>)>,
    cols: usize,
) -> Element<'a, M> {
    let mut lanes: Vec<(usize, Vec<Element<'a, M>>)> =
        (0..cols.max(1)).map(|_| (0, Vec::new())).collect();
    for (weight, card) in cards {
        let lane = lanes
            .iter_mut()
            .min_by_key(|(load, _)| *load)
            .expect("at least one column");
        lane.0 += weight;
        lane.1.push(card);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_least_one_column_and_as_many_as_fit() {
        assert_eq!(columns_for(100.0), 1);
        assert_eq!(columns_for(CARD_WIDTH + 2.0 * PAGE_PADDING), 1);
        assert_eq!(columns_for(1200.0), 4);
    }
}
