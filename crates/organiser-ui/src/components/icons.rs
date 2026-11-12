//! Icons the apps draw themselves: single-colour SVGs on a 16-pixel grid, so they follow the
//! text colour like the symbolic system icons, or take a colour of their own.

use iced::Color;
use iced::widget::svg;

use crate::Element;
use crate::iad::prelude::{Chip, ChipSize};
use crate::iad::{Icon, IconSource, IconView, Role};
use crate::primitives::text::ellipsize;

/// A solid folder: the default icon of a folder, an epic and a list (`Icon::Folder`, bundled by
/// iad, so it takes a colour of its own well).
pub fn folder() -> IconSource {
    Icon::Folder.into()
}

/// A calendar page with today marked: the tasks that are current (`Icon::Calendar`, bundled).
pub fn calendar() -> IconSource {
    Icon::Calendar.into()
}

/// The icon for a stored name (iad's `ICON_CHOICES`); the folder for none, and for a name this
/// version does not know.
pub fn named(name: Option<&str>) -> IconSource {
    Icon::from_choice(name).into()
}

/// An icon `size` across, in `color` when given.
pub fn tinted<'a>(icon: IconSource, size: f32, color: Option<Color>) -> IconView<'a> {
    let view = icon.view(size);
    match color {
        Some(color) => view.style(move |_: &crate::iad::Theme, _| svg::Style { color: Some(color) }),
        None => view,
    }
}

/// A folder as a chip: its icon (the folder unless it has chosen another) and name, in its colour (neutral when it has none): an epic, a list.
pub fn folder_chip<'a, M: Clone + 'a>(
    name: &str,
    color: Option<Color>,
    icon: Option<&str>,
) -> Element<'a, M> {
    let chip = Chip::new(ellipsize(name, 24))
        .icon(named(icon))
        .size(ChipSize::Small);
    match color {
        Some(c) => chip.color(c),
        None => chip.role(Role::Neutral),
    }
    .into()
}
