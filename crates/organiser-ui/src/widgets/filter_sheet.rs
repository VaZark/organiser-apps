//! The layout of a modal sheet (a filter, a list's settings): a title, a close button, the body
//! (scrolling) and a footer: "Done", or "Cancel" and a primary action when `confirm` is given, and
//! "Clear all" when `on_clear` is given. It holds no filter logic; the app supplies the body and the messages.

use crate::components::action::action_button;
use iced::widget::{column, row, space};
use iced::{Alignment, Length};

use crate::Element;
use crate::iad::prelude::*;

pub fn filter_sheet<'a, M: Clone + 'a>(
    title: &str,
    subtitle: Option<String>,
    body: Element<'a, M>,
    on_close: M,
    on_clear: Option<M>,
    confirm: Option<(&str, M)>,
) -> Element<'a, M> {
    let mut heading = column![title_3(title.to_string())]
        .spacing(4)
        .width(Length::Fill);
    if let Some(s) = subtitle {
        heading = heading.push(caption(s));
    }
    let header = row![
        heading,
        tooltip(
            icon_button(Icon::WindowClose).on_press(on_close.clone()),
            "Close",
            TooltipPosition::Bottom
        )
    ]
    .spacing(8)
    .align_y(Alignment::Start);

    let mut footer = row![].spacing(8).align_y(Alignment::Center);
    if let Some(m) = on_clear {
        footer = footer.push(flat_button(text("Clear all")).on_press(m));
    }
    footer = footer.push(space::horizontal());
    match confirm {
        // A sheet that does something (adds an entry): Cancel beside the action.
        Some((label, m)) => {
            footer = footer.push(flat_button(text("Cancel")).on_press(on_close));
            footer = footer.push(action_button(label.to_string()).on_press(m));
        }
        // Closing is not the primary action: the theme's neutral button, not the accent.
        None => footer = footer.push(raised_button(text("Done")).on_press(on_close)),
    }

    column![header, scrolled_window(body).height(Length::Fill), footer]
        .spacing(16)
        .height(Length::Fill)
        .into()
}
