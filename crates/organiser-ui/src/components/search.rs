//! The header search, as in the gallery: an icon at the end of the header bar that turns the
//! header's title into a search entry. Bind `Ctrl+F` to open it and `Esc` to close it.

use iced::Length;

use crate::Element;
use crate::iad::prelude::*;
use crate::primitives::search::balance_width;

/// The id of the entry, which takes focus when the search opens.
pub const SEARCH_ID: &str = "organiser.search";

/// The magnifier button for `header_bar().pack_end(..)`.
pub fn button<'a, M: Clone + 'a>(on_press: M) -> Element<'a, M> {
    let m = Metrics::desktop();
    tooltip(
        iced::widget::button(
            // The bundled artwork is already drawn at the right stroke: no extra size or weight.
            container(Icon::Search.view(m.icon_size)).center(Length::Fixed(m.button_min_height)),
        )
        .padding(0)
        .style(crate::iad::styles::button::flat)
        .on_press(on_press),
        "Search (Ctrl+F)",
        TooltipPosition::Bottom,
    )
}

/// Whether a window is narrow (the toolkit's mobile mode). The header folds its action icons into
/// one menu, and an open search takes over the header.
pub fn narrow(window_width: f32) -> bool {
    crate::iad::breakpoints::is_mobile(window_width)
}

/// Whether an open search takes over the whole header: in a window in mobile mode (the toolkit's
/// own threshold) there is no room for the entry beside the other buttons, so they all step aside,
/// the window's buttons included, and the entry gets the full width to show what is typed.
pub fn takes_over(window_width: f32) -> bool {
    narrow(window_width)
}

/// The arrow at the start of a header whose search has taken over, to leave the search.
pub fn close_button<'a, M: Clone + 'a>(on_press: M) -> Element<'a, M> {
    tooltip(
        icon_button(Icon::GoPrevious).on_press(on_press),
        "Close search",
        TooltipPosition::Bottom,
    )
}

/// An invisible spacer for the start of a header while searching, so that the entry sits in the
/// middle of the header rather than in the middle of what is left after the buttons at the end.
/// See [`balance_width`].
pub fn balance<'a, M: 'a>(end_buttons: usize, start_buttons: usize, room: f32) -> Element<'a, M> {
    iced::widget::space()
        .width(Length::Fixed(balance_width(
            end_buttons,
            start_buttons,
            room,
        )))
        .into()
}

/// The header's title while searching: an entry that has focus. `trailing` sits at the entry's
/// end, e.g. a button that opens filters.
pub fn title<'a, M: Clone + 'a>(
    text: &str,
    placeholder: &str,
    on_input: impl Fn(String) -> M + 'a,
    trailing: Option<Element<'a, M>>,
) -> Element<'a, M> {
    let m = Metrics::desktop();
    let mut field = entry(placeholder, text)
        .id(SEARCH_ID)
        .on_input(on_input)
        .left_section(Icon::Search.view(m.icon_size))
        .metrics(m);
    if let Some(t) = trailing {
        field = field.right_section(t).right_section_width(64.0);
    }
    FocusScope::new(container(clamp(field, 420.0)).center_y(Length::Fill))
        .trap(false)
        .initial_focus(SEARCH_ID)
        .into()
}
