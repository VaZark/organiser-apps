//! Tags and badges as small rounded labels. A chip cannot wrap its text, so a long label is cut
//! with an ellipsis (the full text is in a tooltip) rather than spilling out of its background.

use iced::Color;

use crate::Element;
use crate::iad::Role;
use crate::iad::prelude::{Chip, ChipSize, TooltipPosition, tag, tooltip};
use crate::primitives::text::ellipsize;

/// The most characters a chip shows.
pub const MAX_CHARS: usize = 24;

/// A badge: `label` tinted by `role`, shortened if it is long.
pub fn pill<'a, M: 'a>(label: &str, role: Role) -> Element<'a, M> {
    let shown = ellipsize(label, MAX_CHARS);
    if shown == label {
        tag(shown, role)
    } else {
        tooltip(tag(shown, role), label.to_string(), TooltipPosition::Bottom)
    }
}

/// What a tag shows: `genre:Salsa` is `Salsa`.
fn shown(name: &str) -> &str {
    match name.split_once(':') {
        Some((_, v)) if !v.is_empty() => v,
        _ => name,
    }
}

/// A tag's colour. Stable: the same name (ignoring case) is the same colour everywhere.
pub fn color_of(name: &str) -> Color {
    crate::iad::color::from_seed(name.to_lowercase())
}

fn colored<'a, M: Clone + 'a>(name: &str) -> Chip<'a, M> {
    Chip::new(ellipsize(shown(name), MAX_CHARS))
        .color(color_of(name))
        .size(ChipSize::Small)
}

/// A tag, in its own colour.
pub fn chip<'a, M: Clone + 'a>(name: &str) -> Element<'a, M> {
    colored(name).into()
}

/// A tag as a hashtag, `# name`, in its own colour: how a card shows its tags.
pub fn hashtag<'a, M: Clone + 'a>(name: &str) -> Element<'a, M> {
    Chip::new(format!("# {}", ellipsize(shown(name), MAX_CHARS)))
        .color(color_of(name))
        .size(ChipSize::Small)
        .into()
}

/// A tag with a ✕ that sends `on_remove`.
pub fn removable<'a, M: Clone + 'a>(name: &str, on_remove: M) -> Element<'a, M> {
    colored(name).on_remove(on_remove).into()
}

/// A tag that can be pressed, to pick it.
pub fn pickable<'a, M: Clone + 'a>(name: &str, on_press: M) -> Element<'a, M> {
    colored(name).on_press(on_press).into()
}

/// `+3`: how many tags a crowded row left out.
pub fn more<'a, M: 'a>(n: usize) -> Element<'a, M> {
    tag(format!("+{n}"), Role::Neutral)
}
