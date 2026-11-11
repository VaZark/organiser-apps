//! The log: a vertical daily view. Each row is a day, the date at its start and what was cooked (or
//! is planned) beside it, with the time of day and servings. Only days with a recipe are listed. The "+" in the header logs a meal in a dialog, and so does tapping a day.

use chrono::NaiveDate;
use iced::widget::{column, row};
use iced::{Alignment, Length};
use organiser_ui::Element;
use organiser_ui::components::chip::pill;
use organiser_ui::components::row::selectable;
use organiser_ui::iad::prelude::*;
use organiser_ui::iad::primitives::date::Date as IadDate;
use organiser_ui::widgets::filter_sheet::filter_sheet;
use recipes_schema::mealtime::MealTime;
use recipes_schema::units::pretty;

use crate::app::{App, Message};

impl App {
    /// The days to show, newest (or furthest planned) first.
    fn log_days(&self) -> Vec<NaiveDate> {
        let mut days: Vec<NaiveDate> = self.meals.iter().map(|m| m.date).collect();
        days.sort_unstable_by(|a, b| b.cmp(a));
        days.dedup();
        days
    }

    pub(super) fn calendar_page(&self) -> Element<'_, Message> {
        let mut rows: Vec<Element<'_, Message>> = Vec::new();
        for d in self.log_days() {
            let mut meals: Vec<_> = self.meals.iter().filter(|m| m.date == d).collect();
            // In the order of the day; a meal with no time goes last.
            meals.sort_by_key(|m| (m.time.is_none(), m.time, m.id));
            let date_label = if d == self.today {
                "Today".to_string()
            } else {
                d.format("%a %-d %b").to_string()
            };
            let date: Element<'_, Message> = column![
                text(date_label).size(14),
                caption(d.format("%Y").to_string())
            ]
            .width(96)
            .into();
            let mut items = column![].spacing(4).width(Length::Fill);
            if meals.is_empty() {
                items = items.push(caption("—"));
            }
            for m in meals {
                let name: Element<'_, Message> = match m.recipe_id {
                    Some(id) => flat_button(text(m.title.clone()))
                        .on_press(Message::Open(id))
                        .into(),
                    None => text(m.title.clone()).into(),
                };
                let mut line = row![].spacing(8).align_y(Alignment::Center);
                // The time of day in a column of its own, so recipes line up.
                line = line.push(
                    container(match m.time {
                        Some(t) => caption(t.label()).into(),
                        None => Element::from(space()),
                    })
                    .width(70),
                );
                line = line.push(container(name).width(Length::Fill));
                if m.servings > 1.0 {
                    line = line.push(pill(
                        &format!("{} servings", pretty(m.servings)),
                        Role::Neutral,
                    ));
                }
                if m.is_planned(self.today) {
                    line = line.push(pill("planned", Role::Neutral));
                }
                items = items
                    .push(line.push(icon_button(Icon::Delete).on_press(Message::MealDelete(m.id))));
            }
            rows.push(selectable(
                row![date, items].spacing(12).padding([6, 12]),
                false,
                Message::OpenLog(Some(d), None),
            ));
        }
        // The same container as the fridge: a centred column of limited width, a titled group, headers
        // over the columns, then the list.
        let mut group = preferences_group().flat(true).title("Cooked and planned");
        group = group.description(if rows.is_empty() {
            "Nothing logged yet. Press + to log a meal."
        } else {
            "Tap a day to log another meal on it."
        });
        if !rows.is_empty() {
            group = group
                .push(
                    row![
                        text("Date").size(15).width(96),
                        row![
                            text("Meal").size(15).width(70),
                            container(text("Recipe").size(15)).padding([0, 10])
                        ]
                        .spacing(8),
                    ]
                    .spacing(12)
                    .padding([4, 12]),
                )
                .push(boxed_list(rows));
        }
        scrolled_window(clamp(
            container(group).padding([16, 24]),
            organiser_ui::widgets::details_dock::READABLE_WIDTH + 200.0,
        ))
        .height(Length::Fill)
        .into()
    }

    /// The dialog that logs a meal: the day (today by default), the recipe, then the time of day
    /// and the servings.
    pub(super) fn log_panel(&self) -> Element<'_, Message> {
        let date = EntryRow::new(
            "Date",
            EntryInput::Date(
                DateInput::new(&self.log_text, self.log_view, Message::LogDateText)
                    .selected(IadDate::parse(&self.log_text))
                    .on_pick(Message::LogDatePick)
                    .on_view(Message::LogDateView)
                    .width(Length::Fill),
            ),
        )
        .description("Today or earlier is cooked; a later day is planned.")
        .flat(true);

        let recipe = EntryRow::select(
            "Recipe",
            &self.recipe_choices,
            self.cal_recipe.clone(),
            Message::CalRecipe,
        )
        .flat(true);

        let time = EntryRow::select(
            "Meal",
            &self.meal_choices,
            Some(self.log_time),
            Message::LogTime,
        )
        .flat(true);
        let servings = EntryRow::new(
            "Servings",
            EntryInput::Text(
                Entry::new("1", self.log_servings.as_str()).on_input(Message::LogServings),
            ),
        )
        .flat(true);

        let _ = MealTime::ALL;
        filter_sheet(
            "Log a meal",
            None,
            column![date, recipe, row![time, servings].spacing(12)]
                .spacing(12)
                .into(),
            Message::CloseLog,
            None,
            Some(("Log meal", Message::LogSave)),
        )
    }
}
