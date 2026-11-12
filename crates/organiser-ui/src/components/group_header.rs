//! The heading of a group in a list shown in sections: a folder's or an epic's icon in its colour,
//! its name, how many items it holds and, if it can be edited, a button for that. The items that
//! belong to no group are headed "Unassigned", with no icon.

use iced::widget::{column, container, row, text};
use iced::{Alignment, Color, Length};

use crate::Element;
use crate::components::icons::tinted;
use crate::iad::IconSource;
use crate::iad::prelude::*;
use crate::primitives::text::ellipsize;

/// What the items that belong to no group are headed.
pub const UNASSIGNED: &str = "Unassigned";

/// A group heading. `icon` is the group's icon with its colour (none for "Unassigned"),
/// `description` what it is for (its first line, shown small), `count`
/// how many items there are (none leaves it out, for a compact heading), and `edit` the message
/// and tooltip of an edit button at the end.
pub fn group_header<'a, M: Clone + 'a>(
    icon: Option<(IconSource, Option<Color>)>,
    title: &str,
    description: Option<&str>,
    count: Option<usize>,
    edit: Option<(M, &'static str)>,
) -> Element<'a, M> {
    let mut head = row![].spacing(8).align_y(Alignment::Center);
    if let Some((source, color)) = icon {
        head = head.push(tinted(source, 18.0, color));
    }
    // The name, with what the group is for under it, small, when it says.
    let mut name = column![text(title.to_string()).size(15)].spacing(1);
    if let Some(line) = description {
        name = name.push(
            text(ellipsize(line, 110))
                .size(11)
                .style(crate::iad::styles::text::dim),
        );
    }
    // The name takes the room, so the count sits at the end, beside the edit button.
    head = head.push(container(name).width(Length::Fill));
    if let Some(n) = count {
        head = head.push(caption(n.to_string()));
        if let Some((message, tip)) = edit {
            head = head.push(tooltip(
                icon_button(Icon::Edit).on_press(message),
                tip,
                TooltipPosition::Bottom,
            ));
        }
    }
    container(head).padding([4, 4]).into()
}
