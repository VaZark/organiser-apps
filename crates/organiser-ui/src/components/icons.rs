//! Icons the apps draw themselves: single-colour SVGs on a 16-pixel grid, so they follow the
//! text colour like the symbolic system icons, or take a colour of their own.

use std::sync::OnceLock;

use iced::Color;
use iced::widget::svg;

use crate::Element;
use crate::iad::prelude::{Chip, ChipSize};
use crate::iad::{IconSource, IconView, Role};
use crate::primitives::text::ellipsize;

/// A calendar page with today marked: the tasks that are current.
const CALENDAR: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16">
<path fill-rule="evenodd" d="M4.5 1a.5.5 0 0 1 .5.5V2h6v-.5a.5.5 0 0 1 1 0V2h.5A2.5 2.5 0 0 1 15 4.5v8a2.5 2.5 0 0 1-2.5 2.5h-9A2.5 2.5 0 0 1 1 12.5v-8A2.5 2.5 0 0 1 3.5 2H4v-.5a.5.5 0 0 1 .5-.5zM2 6.5v6A1.5 1.5 0 0 0 3.5 14h9a1.5 1.5 0 0 0 1.5-1.5v-6zM3.5 3A1.5 1.5 0 0 0 2 4.5V5.5h12v-1A1.5 1.5 0 0 0 12.5 3z"/>
<rect x="8.5" y="8.5" width="3.5" height="3.5" rx="0.9"/>
<circle cx="4.9" cy="9.2" r="0.8"/>
<circle cx="4.9" cy="11.8" r="0.8"/>
</svg>"#;

/// A solid folder: an epic, tinted with its colour.
const FOLDER: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16">
<path d="M2.5 2A1.5 1.5 0 0 0 1 3.5v9A1.5 1.5 0 0 0 2.5 14h11a1.5 1.5 0 0 0 1.5-1.5V5.5A1.5 1.5 0 0 0 13.5 4H8.2a.5.5 0 0 1-.4-.2L6.7 2.4A1.5 1.5 0 0 0 5.5 2z"/>
</svg>"#;

fn handle(cell: &'static OnceLock<svg::Handle>, source: &'static str) -> IconSource {
    cell.get_or_init(|| svg::Handle::from_memory(source.as_bytes()))
        .clone()
        .into()
}

pub fn calendar() -> IconSource {
    static H: OnceLock<svg::Handle> = OnceLock::new();
    handle(&H, CALENDAR)
}

pub fn folder() -> IconSource {
    static H: OnceLock<svg::Handle> = OnceLock::new();
    handle(&H, FOLDER)
}

/// An icon `size` across, in `color` when given.
pub fn tinted<'a>(icon: IconSource, size: f32, color: Option<Color>) -> IconView<'a> {
    let view = icon.view(size);
    match color {
        Some(color) => view.style(move |_: &crate::iad::Theme, _| svg::Style { color: Some(color) }),
        None => view,
    }
}

/// An epic as a chip: its folder and name, in the epic's colour (neutral when it has none).
pub fn epic_chip<'a, M: Clone + 'a>(name: &str, color: Option<Color>) -> Element<'a, M> {
    let chip = Chip::new(ellipsize(name, 24))
        .icon(folder())
        .size(ChipSize::Small);
    match color {
        Some(c) => chip.color(c),
        None => chip.role(Role::Neutral),
    }
    .into()
}
