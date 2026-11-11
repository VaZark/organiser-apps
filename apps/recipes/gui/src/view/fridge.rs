//! The fridge: food you have, with an optional expiry date. It stands alone; the shopping list
//! (from the header) is what reads it.

use iced::widget::{column, container, row};
use iced::{Alignment, Length};
use organiser_ui::Element;
use organiser_ui::iad::prelude::*;

use crate::app::{App, Message, parse_qty};
use organiser_ui::components::tab_scope::tab_scope;
use organiser_ui::iad::widgets::ReorderableList;

// Columns shared by the headers, the rows and the add row, so everything lines up.
const AMOUNT: f32 = 146.0;
const DATE: f32 = 120.0;
const ACTIONS: f32 = 76.0;
const COLUMN_GAP: f32 = 12.0;
/// The width the list gives each row's drag grip, kept free in the header and the add row.
const GRIP: f32 = 48.0;

/// The fields of one fridge entry, as entries: the same row to change an entry and to add one,
/// with the actions at its end.
#[allow(clippy::too_many_arguments)]
fn fields<'a>(
    name: &'a str,
    qty: &'a str,
    unit: &'a str,
    expires: &'a str,
    on_name: impl Fn(String) -> Message + 'a,
    on_qty: impl Fn(String) -> Message + 'a,
    on_unit: impl Fn(String) -> Message + 'a,
    on_expires: impl Fn(String) -> Message + 'a,
    actions: iced::widget::Row<'a, Message, organiser_ui::iad::Theme>,
    on_submit: Option<Message>,
) -> iced::widget::Row<'a, Message, organiser_ui::iad::Theme> {
    let mut food = entry("Food", name).on_input(on_name).width(Length::Fill);
    if let Some(m) = on_submit {
        food = food.on_submit(m);
    }
    row![
        food,
        row![
            entry("Qty", qty).on_input(on_qty).width(70),
            entry("Unit", unit).on_input(on_unit).width(70),
        ]
        .spacing(6)
        .width(AMOUNT),
        entry("YYYY-MM-DD", expires)
            .on_input(on_expires)
            .width(DATE),
        container(actions).width(ACTIONS).align_x(Alignment::End),
    ]
    .spacing(COLUMN_GAP)
    .padding([6, 12])
    .align_y(Alignment::Center)
}

impl App {
    pub(super) fn fridge_page(&self) -> Element<'_, Message> {
        let mut g = preferences_group().flat(true).title("In the fridge").description(
            "What you have. An expiry date is optional: food past it stops counting when a shopping list is prepared. Each entry has its own edit and delete buttons.",
        );
        // Columns shared by the headers, the rows and the add row, so everything lines up.
        // Laid out like a list row: the columns, then the width the list keeps for its grip.
        let header = row![
            row![
                text("Food").size(15).width(Length::Fill),
                text("Amount").size(15).width(AMOUNT),
                text("Expires").size(15).width(DATE),
                space().width(ACTIONS),
            ]
            .spacing(COLUMN_GAP)
            .padding([4, 12])
            .width(Length::Fill),
            space().width(GRIP),
        ];
        g = g.push(header);
        // The saved order, with a grip at the end of each row to drag it elsewhere.
        if !self.stock.is_empty() {
            g = g.push(
                ReorderableList::of(&self.stock_order, |id| self.stock_line(*id))
                    .controls(false)
                    .on_event(Message::FridgeList),
            );
        }
        if self.stock.is_empty() {
            g = g.push(caption("Nothing yet. Add what you have below."));
        }
        // The add row is an edit row with one action: add.
        let (name, qty, unit, exp) = &self.stock_new;
        g = g.push(row![
            fields(
                name,
                qty,
                unit,
                exp,
                Message::StockName,
                Message::StockQty,
                Message::StockUnit,
                Message::StockExpires,
                row![tooltip(
                    icon_button(Icon::ListAdd)
                        .on_press_maybe((!name.trim().is_empty()).then_some(Message::StockAdd)),
                    "Add to the fridge",
                    TooltipPosition::Bottom
                )],
                Some(Message::StockAdd),
            )
            .width(Length::Fill),
            space().width(GRIP),
        ]);
        // The shopping list is prepared from the fridge and recipes, so its button is here.
        let actions = row![
            space().width(Length::Fill),
            tooltip(
                flat_button(row![Icon::FlatList.view(16.0), text("Shopping list")].spacing(6))
                    .on_press(Message::OpenShop),
                "Prepare a shopping list from recipes and the fridge",
                TooltipPosition::Bottom,
            ),
        ];
        scrolled_window(clamp(
            iced::widget::container(tab_scope(column![actions, g].spacing(16))).padding([16, 24]),
            organiser_ui::widgets::details_dock::READABLE_WIDTH + 200.0,
        ))
        .height(Length::Fill)
        .into()
    }

    /// One fridge entry as a row: read only, or its fields while it is the one being edited.
    fn stock_line(&self, id: i64) -> Element<'_, Message> {
        let Some(s) = self.stock.iter().find(|s| s.id == id) else {
            return space().into();
        };
        let line = if self.fridge_edit == Some(id) {
            // This one entry is being changed, in the same fields the add row has; finishing
            // it goes back to reading.
            fields(
                &s.name,
                &s.qty,
                &s.unit,
                &s.expires,
                move |v| Message::StockRowName(id, v),
                move |v| Message::StockRowQty(id, v),
                move |v| Message::StockRowUnit(id, v),
                move |v| Message::StockRowExpires(id, v),
                row![
                    tooltip(
                        icon_button(Icon::CheckCircle).on_press(Message::FridgeEdit(None)),
                        "Done",
                        TooltipPosition::Bottom
                    ),
                    icon_button(Icon::Delete).on_press(Message::StockDelete(id)),
                ]
                .spacing(4),
                Some(Message::FridgeEdit(None)),
            )
        } else {
            let amount = super::parts::amount(parse(&s.qty), &s.unit);
            row![
                text(s.name.clone()).width(Length::Fill),
                text(if amount.is_empty() {
                    "some".to_string()
                } else {
                    amount
                })
                .width(AMOUNT),
                text(s.expires.clone()).width(DATE),
                row![
                    tooltip(
                        icon_button(Icon::Edit).on_press(Message::FridgeEdit(Some(id))),
                        "Edit",
                        TooltipPosition::Bottom
                    ),
                    icon_button(Icon::Delete).on_press(Message::StockDelete(id)),
                ]
                .spacing(4)
                .width(ACTIONS),
            ]
        }
        .spacing(COLUMN_GAP)
        .padding([6, 12])
        .align_y(Alignment::Center);
        line.into()
    }
}

fn parse(q: &str) -> Option<f64> {
    parse_qty(q).ok().flatten()
}
