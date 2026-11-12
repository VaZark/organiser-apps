//! One row of a list of items, laid out like the Todo list: the title block on the left, the chips
//! beside it (or under it when the list is narrow, so the title keeps the room instead of
//! breaking one word per line), and an optional control at the end, such as a heart. The whole
//! row is one button; controls inside it stay independent.

use iced::widget::{column, container, row};
use iced::{Alignment, Length};

use crate::Element;
use crate::components::row::selectable;

/// The most width the chips take beside a title before they wrap.
pub const CHIPS_WIDTH: f32 = 270.0;

/// Below this list width the chips go under the title.
pub const STACK_BELOW: f32 = 620.0;

/// Whether a list `width` wide is too narrow for chips beside the titles.
pub fn stacked(width: f32) -> bool {
    width < STACK_BELOW
}

/// A row: `title` (usually the title over a dim line), `chips` (a `row` that wraps), and `trail`
/// at the end. `stacked` puts the chips under the title.
pub fn list_item<'a, M: Clone + 'a>(
    title: impl Into<Element<'a, M>>,
    chips: Option<Element<'a, M>>,
    trail: Option<Element<'a, M>>,
    stacked: bool,
    selected: bool,
    on_press: M,
) -> Element<'a, M> {
    let title = container(title.into()).width(Length::Fill).padding([6, 8]);
    let main: Element<'a, M> = match (chips, stacked) {
        (None, _) => title.into(),
        (Some(chips), true) => column![title, container(chips).padding([0, 8])]
            .spacing(4)
            .width(Length::Fill)
            .into(),
        (Some(chips), false) => row![title, container(chips).max_width(CHIPS_WIDTH)]
            .spacing(6)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .into(),
    };
    let mut line = row![main].spacing(6).align_y(Alignment::Center);
    if let Some(t) = trail {
        line = line.push(t);
    }
    // Room at both edges: the chips and the control must not touch them.
    selectable(line.padding([4, 12]), selected, on_press)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_narrow_list_stacks_its_chips() {
        assert!(stacked(400.0));
        assert!(!stacked(900.0));
        assert!(stacked(STACK_BELOW - 1.0) && !stacked(STACK_BELOW));
    }
}
