//! An icon in a header that opens a popover of choices, one of which is selected: the sort order,
//! say. It is iad's `dropdown` with an icon for its trigger, so it looks and behaves like one.

use iced::Length;
use iced::widget::container;

use crate::Element;
use crate::iad::prelude::*;

/// `icon` is the button, `label` its tooltip, and `options` the choices, with `selected` ticked.
/// `open_initially` shows the popover open on first draw (for screenshots).
pub fn choice_menu<'a, T, M>(
    icon: Icon,
    label: &str,
    options: &'a [T],
    selected: Option<T>,
    on_select: impl Fn(T) -> M + 'a,
    open_initially: bool,
) -> Element<'a, M>
where
    T: ToString + PartialEq + Clone + 'a,
    M: Clone + 'a,
{
    let m = Metrics::desktop();
    let items: Vec<Element<'a, M>> = options
        .iter()
        .map(|o| {
            menu_item(
                o.to_string(),
                selected.as_ref() == Some(o),
                on_select(o.clone()),
            )
        })
        .collect();
    let trigger = container(icon.view(m.icon_size))
        .center(Length::Fixed(m.button_min_height - m.button_padding.y()));
    let menu = Popover::new(trigger, popover(items, 220.0))
        .appearance(PopoverAppearance::Flat)
        .align(PopoverAlign::End)
        .open_initially(open_initially);
    tooltip(menu, label.to_string(), TooltipPosition::Bottom)
}
