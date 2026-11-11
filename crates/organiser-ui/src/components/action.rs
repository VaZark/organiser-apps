//! The button that confirms or adds ("Create note", "Add", "Cooked it"). Neutral, like the rest of
//! the apps: the raised grey surface, never a blue fill.

use crate::Element;
use crate::iad::prelude::*;

/// A raised neutral button with a text label; chain `.on_press(..)` as with any button.
pub fn action_button<'a, M: 'a>(
    label: impl ToString,
) -> iced::widget::Button<'a, M, crate::iad::Theme> {
    raised_button(text(label.to_string()))
}

/// The same, as an element that is disabled until `on_press` is `Some`.
pub fn action_maybe<'a, M: Clone + 'a>(
    label: impl ToString,
    on_press: Option<M>,
) -> Element<'a, M> {
    action_button(label).on_press_maybe(on_press).into()
}
