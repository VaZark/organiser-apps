//! The header title that switches page: the page's name with a small chevron, flat until hovered.
//! In a narrow window the sidebar is gone and this takes its place in the header bar. Two forms
//! share one look:
//!
//! * [`title_button`]: a press opens something of the app's own (Faves opens its lists dialog);
//! * [`page_switcher`]: a press opens a popover with the pages to choose from, the current one
//!   ticked (Recipes: Recipes, Log, Fridge).

use std::fmt::Display;

use iced::Alignment;
use iced::widget::row;

use crate::Element;
use crate::iad::prelude::*;
use crate::iad::widgets::{Popover, PopoverAppearance, menu_item, popover};

/// The page name and its chevron.
fn trigger<'a, M: 'a>(label: String) -> Element<'a, M> {
    row![text(label), Icon::GoDown.view(12.0)]
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
}

/// A flat title button with a tooltip; `on_press` opens whatever the app switches with.
pub fn title_button<'a, M: Clone + 'a>(
    label: impl Into<String>,
    tip: &'static str,
    on_press: M,
) -> Element<'a, M> {
    tooltip(
        flat_button(trigger(label.into())).on_press(on_press),
        tip,
        TooltipPosition::Bottom,
    )
}

/// A flat title that opens a popover of `pages`, `current` ticked; choosing one sends
/// `on_select(page)`.
pub fn page_switcher<'a, T, M>(
    pages: &'a [T],
    current: T,
    on_select: impl Fn(T) -> M + 'a,
) -> Element<'a, M>
where
    T: Display + PartialEq + Clone + 'a,
    M: Clone + 'a,
{
    let items: Vec<Element<'a, M>> = pages
        .iter()
        .map(|p| menu_item(p.to_string(), *p == current, on_select(p.clone())))
        .collect();
    Popover::new(trigger(current.to_string()), popover(items, 200.0))
        .appearance(PopoverAppearance::Flat)
        .into()
}
