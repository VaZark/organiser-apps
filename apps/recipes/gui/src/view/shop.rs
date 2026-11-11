//! Prepare a shopping list: pick recipes and how many portions of each, and it works out what to
//! buy from what the fridge has (food past its date does not count). The list is remembered, with
//! the day it last changed. Export it as Markdown task lines for a Todo note, or save it as a file.
//! The close button leaves; there is no "Done".

use chrono::DateTime;
use iced::widget::{column, row};
use iced::{Alignment, Length};
use organiser_ui::Element;
use organiser_ui::components::action::action_maybe;
use organiser_ui::iad::prelude::*;
use recipes_schema::shopping::Status;
use recipes_schema::units::pretty;

use super::parts::item_line;
use crate::app::{App, Message};

impl App {
    pub(super) fn shop_panel(&self) -> Element<'_, Message> {
        let mut picks = preferences_group()
            .flat(true)
            .title("Recipes")
            .description("Pick what you will cook and how many portions of each.");
        for r in &self.shop {
            let id = r.recipe.id;
            let note = match r.servings {
                Some(s) => format!("makes {}", pretty(s)),
                None => "no servings set".to_string(),
            };
            picks = picks.push(
                row![
                    column![text(r.recipe.title.clone()), caption(note)].width(Length::Fill),
                    stepper_portions(r.portions, id),
                    icon_button(Icon::Delete).on_press(Message::ShopRemove(id)),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            );
        }
        picks = picks.push(
            row![
                dropdown(
                    &self.recipe_choices,
                    self.shop_pick.clone(),
                    Message::ShopPick
                ),
                action_maybe("Add", self.shop_pick.is_some().then_some(Message::ShopAdd)),
                space().width(Length::Fill),
                flat_button(text("Add planned meals")).on_press(Message::ShopPlanned),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );

        let to_buy: Vec<_> = self
            .shop_items
            .iter()
            .filter(|i| i.status != Status::Enough)
            .collect();
        let have = self.shop_items.len() - to_buy.len();
        let mut list = preferences_group().flat(true).title("To buy");
        list = list.description(if self.shop.is_empty() {
            "Nothing picked yet.".to_string()
        } else if to_buy.is_empty() {
            "The fridge covers everything.".to_string()
        } else if have > 0 {
            format!("{have} more are already in the fridge.")
        } else {
            "Compared with the fridge.".to_string()
        });
        for i in &to_buy {
            list = list.push(text(item_line(i)));
        }

        // Last changed, remembered with the list.
        let changed = self
            .shop_changed
            .and_then(|t| DateTime::from_timestamp(t, 0))
            .map(|t| format!("Last changed {}", t.format("%-d %B %Y")));
        let mut heading = column![title_3("Shopping list".to_string())]
            .spacing(4)
            .width(Length::Fill);
        if let Some(c) = changed {
            heading = heading.push(caption(c));
        }
        let header = row![
            heading,
            tooltip(
                icon_button(Icon::WindowClose).on_press(Message::CloseShop),
                "Close",
                TooltipPosition::Bottom
            ),
        ]
        .spacing(8)
        .align_y(Alignment::Start);

        let ready = !to_buy.is_empty();
        let mut footer = row![].spacing(8).align_y(Alignment::Center);
        if !self.shop.is_empty() {
            footer = footer.push(flat_button(text("Clear")).on_press(Message::ShopClear));
        }
        footer = footer.push(space().width(Length::Fill));
        footer = footer.push(tooltip(
            flat_button(row![Icon::Note.view(16.0), text("Export as MD")].spacing(6))
                .on_press_maybe(ready.then_some(Message::ShopCopy)),
            "Copy as Markdown tasks, to paste into a Todo note",
            TooltipPosition::Top,
        ));
        footer = footer.push(tooltip(
            raised_button(row![Icon::Archive.view(16.0), text("Save")].spacing(6))
                .on_press_maybe(ready.then_some(Message::ShopSave)),
            "Save the list as a Markdown file",
            TooltipPosition::Top,
        ));

        column![
            header,
            scrolled_window(column![picks, list].spacing(20)).height(Length::Fill),
            footer
        ]
        .spacing(16)
        .height(Length::Fill)
        .into()
    }
}

/// `− 10 portions +`; portions are whole numbers, at least 1.
fn stepper_portions(portions: f64, id: i64) -> Element<'static, Message> {
    row![
        flat_button(text("−"))
            .on_press_maybe((portions > 1.0).then_some(Message::ShopPortions(id, portions - 1.0))),
        text(format!("{} portions", pretty(portions)))
            .width(90)
            .align_x(Alignment::Center),
        flat_button(text("+")).on_press(Message::ShopPortions(id, portions + 1.0)),
    ]
    .spacing(4)
    .align_y(Alignment::Center)
    .into()
}
