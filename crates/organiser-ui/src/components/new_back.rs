//! The header button at the start of an app with an edit page: a "+" that opens the page on a new
//! entry, and, while the page is open (new entry or existing one), a back arrow that closes it.

use crate::Element;
use crate::iad::prelude::*;

/// `editing` is whether the edit page is open. `new_tip` names what "+" makes, such as
/// "New note (Ctrl+N)".
pub fn new_or_back<'a, M: Clone + 'a>(
    editing: bool,
    new_tip: &'static str,
    on_new: M,
    on_back: M,
) -> Element<'a, M> {
    if editing {
        tooltip(
            icon_button(Icon::GoPrevious).on_press(on_back),
            "Back",
            TooltipPosition::Bottom,
        )
    } else {
        tooltip(
            icon_button(Icon::ListAdd).on_press(on_new),
            new_tip,
            TooltipPosition::Bottom,
        )
    }
}
