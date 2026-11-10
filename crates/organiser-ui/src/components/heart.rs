//! The favourite heart: a plain icon at the edge of a card or a row. Filled for a favourite, an
//! outline for one that is not. There is no button fill; the heart itself is the state.

use iced::Length;
use iced::widget::{button, container};

use crate::Element;
use crate::iad::prelude::{TooltipPosition, tooltip};
use crate::iad::styles::button::flat;
use crate::iad::{Icon, Metrics};

/// The size of the heart's button: a little under a full button, so a card's title row stays tight.
const HEART_BOX: f32 = 26.0;

/// iad's own hearts: filled for a favourite, an outline for one that is not.
fn heart_icon(favorite: bool) -> Icon {
    if favorite {
        Icon::HeartFilled
    } else {
        Icon::Heart
    }
}

/// `on_press` should flip the favourite.
pub fn heart<'a, M: Clone + 'a>(favorite: bool, on_press: M) -> Element<'a, M> {
    let m = Metrics::desktop();
    tooltip(
        button(
            container(heart_icon(favorite).view(m.icon_size + 2.0))
                .center(Length::Fixed(HEART_BOX)),
        )
        .padding(0)
        .style(flat)
        .on_press(on_press),
        if favorite {
            "Remove from favourites"
        } else {
            "Add to favourites"
        },
        TooltipPosition::Bottom,
    )
}
