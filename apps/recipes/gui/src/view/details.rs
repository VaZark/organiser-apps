//! The open recipe. It opens to read: photos, ingredients with what is missing, numbered steps and
//! the personal note. Edit switches the same page to fields; every edit is saved as it is made.

use iced::widget::{column, row, text_editor};
use iced::{Alignment, Length};
use organiser_ui::Element;
use organiser_ui::components::action::action_button;
use organiser_ui::components::chip::{hashtag, pill};
use organiser_ui::components::heart::heart;
use organiser_ui::iad::prelude::*;
use organiser_ui::widgets::markdown_field::MarkdownField;
use organiser_ui::widgets::tag_editor::tag_editor;

use super::grid::stars;
use super::parts::{amount, strip, strip_edit};
use crate::app::{App, Details, IngRow, Message};

fn field<'a>(label: &'a str, control: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    column![caption(label), control.into()]
        .spacing(4)
        .width(Length::Fill)
        .into()
}

impl App {
    pub(super) fn details_pane(&self) -> Element<'_, Message> {
        let Some(d) = &self.details else {
            return space().into();
        };
        let body = if d.edit {
            // Tab moves through the fields of the form.
            organiser_ui::components::tab_scope::tab_scope(self.edit_view(d))
        } else {
            self.read_view(d)
        };
        scrolled_window(clamp(
            container(body).padding(16),
            organiser_ui::widgets::details_dock::READABLE_WIDTH,
        ))
        .height(Length::Fill)
        .into()
    }

    fn read_view<'a>(&'a self, d: &'a Details) -> Element<'a, Message> {
        let e = &d.entry;
        let theme = self.theme();
        let mut sub: Vec<String> = Vec::new();
        if let Some(r) = e.rating {
            sub.push(stars(r));
        }
        if let Some(n) = e.servings {
            sub.push(format!("serves {}", recipes_schema::units::pretty(n)));
        }
        let mut col = column![
            row![
                text(e.title.clone()).size(24).width(Length::Fill),
                heart(e.favorite, Message::DFavorite(!e.favorite)),
                tooltip(
                    icon_button(Icon::Edit).on_press(Message::DEdit(true)),
                    "Edit",
                    TooltipPosition::Bottom
                ),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(10)
        .width(Length::Fill);
        if !sub.is_empty() {
            col = col.push(caption(sub.join(" · ")));
        }
        if !e.tags.is_empty() {
            col = col.push(row(e.tags.iter().map(|t| hashtag(t))).spacing(4).wrap());
        }
        if !e.photos.is_empty() {
            col = col.push(strip(&e.photos, 200.0));
        }

        // Cooked it today, or put it on the log for another day.
        let history = match d.history.iter().find(|m| !m.is_planned(self.today)) {
            Some(m) => {
                let n = d
                    .history
                    .iter()
                    .filter(|m| !m.is_planned(self.today))
                    .count();
                format!(
                    "Cooked {n} time{}, last on {}",
                    if n == 1 { "" } else { "s" },
                    m.date
                )
            }
            None => "Not cooked yet".to_string(),
        };
        col = col.push(
            column![
                row![
                    action_button("Cooked it today").on_press(Message::CookedToday),
                    flat_button(row![Icon::CheckCircle.view(16.0), text("Log it…")].spacing(6))
                        .on_press(Message::PlanIt),
                ]
                .spacing(8),
                caption(history),
            ]
            .spacing(8),
        );

        // Ingredients: quantity, name, and whether it is optional.
        if !d.ings.is_empty() {
            let mut g = preferences_group().flat(true).title("Ingredients");
            for i in &d.ings {
                let mut line = row![
                    text(amount(parse(&i.qty), &i.unit)).width(90),
                    text(i.name.clone()),
                ]
                .spacing(8)
                .align_y(Alignment::Center);
                if i.optional {
                    line = line.push(pill("optional", Role::Neutral));
                }
                g = g.push(line);
            }
            col = col.push(g);
        }

        if !d.steps.is_empty() {
            let mut g = preferences_group().flat(true).title("Steps");
            for (n, s) in d.steps.iter().enumerate() {
                let mut body = column![RichText::new(&theme, &s.doc).on_link(Message::OpenLink)]
                    .spacing(6)
                    .width(Length::Fill);
                if !s.photos.is_empty() {
                    body = body.push(strip(&s.photos, 160.0));
                }
                g = g.push(
                    row![text(format!("{}.", n + 1)).size(16).width(28), body]
                        .spacing(8)
                        .align_y(Alignment::Start),
                );
            }
            col = col.push(g);
        }
        if !e.notes.is_empty() {
            col = col.push(
                preferences_group().flat(true).title("My notes").push(
                    RichText::new(&theme, &d.doc)
                        .on_link(Message::OpenLink)
                        .on_task(Message::DTask),
                ),
            );
        }
        if let Some(err) = &d.error {
            col = col.push(tag(err.clone(), Role::Error));
        }
        col.into()
    }

    fn edit_view<'a>(&'a self, d: &'a Details) -> Element<'a, Message> {
        let e = &d.entry;
        let mut col = column![
            row![
                caption(if d.is_new() {
                    "New recipe".to_string()
                } else {
                    "Editing".to_string()
                })
                .width(Length::Fill),
                if d.is_new() {
                    Element::from(space())
                } else {
                    flat_button(text("Done"))
                        .on_press(Message::DEdit(false))
                        .into()
                },
            ]
            .align_y(Alignment::Center),
            row![
                entry("Title", d.title.as_str())
                    .on_input(Message::DTitle)
                    .width(Length::Fill),
                heart(e.favorite, Message::DFavorite(!e.favorite))
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            tag_editor(
                &e.tags,
                d.adding_tag,
                &d.tag_text,
                &self.tag_names,
                Message::DTagStart,
                Message::DTagText,
                &|t| Message::DTagAdd(t),
                &|t| Message::DTagRemove(t),
            ),
        ]
        .spacing(14)
        .width(Length::Fill);

        {
            let mut stars_row = row![].spacing(2).align_y(Alignment::Center);
            for n in 1..=5u8 {
                let on = e.rating.is_some_and(|r| n <= r);
                let next = if e.rating == Some(n) { None } else { Some(n) };
                stars_row = stars_row.push(
                    flat_button(text(if on { "★" } else { "☆" }).size(22))
                        .on_press(Message::DRating(next)),
                );
            }
            col = col.push(field("Rating", stars_row));
        }
        col = col.push(field(
            "Serves (portions it makes; scales shopping lists)",
            entry(
                "e.g. 4",
                e.servings
                    .map(recipes_schema::units::pretty)
                    .unwrap_or_default(),
            )
            .on_input(Message::DServings)
            .width(100),
        ));

        if d.is_new() {
            col = col.push(caption(
                "Photos, ingredients and steps come right after you create it.",
            ));
        } else {
            col = col
                .push(
                    preferences_group()
                        .flat(true)
                        .title("Photos")
                        .push(strip_edit(&e.photos, None, 120.0)),
                )
                .push(self.ingredients_editor(d))
                .push(self.steps_editor(d));
        }
        col = col.push(
            MarkdownField::new(
                "My notes",
                &d.notes,
                &d.doc,
                self.theme(),
                Message::DNotes,
                Message::DPreview,
            )
            .preview(d.preview)
            .placeholder("What you changed, what went wrong, who liked it. Markdown works.")
            .on_task(Message::DTask)
            .on_link(Message::OpenLink),
        );
        if let Some(err) = &d.error {
            col = col.push(tag(err.clone(), Role::Error));
        }
        col = if d.is_new() {
            let ready = !d.title.trim().is_empty();
            col.push(
                row![
                    flat_button(text("Cancel")).on_press(Message::CloseDetails),
                    action_button("Create recipe")
                        .on_press_maybe(ready.then_some(Message::DCreate)),
                ]
                .spacing(8),
            )
        } else {
            col.push(destructive_button(text("Delete recipe")).on_press(Message::DDelete))
        };
        col.into()
    }

    fn ingredients_editor<'a>(&'a self, d: &'a Details) -> Element<'a, Message> {
        let mut g = preferences_group().flat(true).title("Ingredients").description(
            "Leave the quantity empty for “to taste”. Optional ingredients are never counted as missing.",
        );
        for i in &d.ings {
            g = g.push(ing_row(i, false));
        }
        g = g.push(ing_row(&d.new_ing, true));
        g.into()
    }

    fn steps_editor<'a>(&'a self, d: &'a Details) -> Element<'a, Message> {
        let mut g = preferences_group().flat(true).title("Steps");
        let last = d.steps.len().saturating_sub(1);
        for (n, s) in d.steps.iter().enumerate() {
            let id = s.id;
            g = g.push(
                column![
                    row![
                        caption(format!("Step {}", n + 1)).width(Length::Fill),
                        icon_button(Icon::GoUp)
                            .on_press_maybe((n > 0).then_some(Message::StepMove(id, -1))),
                        icon_button(Icon::GoDown)
                            .on_press_maybe((n < last).then_some(Message::StepMove(id, 1))),
                        icon_button(Icon::Delete).on_press(Message::StepDelete(id)),
                    ]
                    .spacing(4)
                    .align_y(Alignment::Center),
                    text_view(&s.content)
                        .on_action(move |a: text_editor::Action| Message::StepText(id, a))
                        .placeholder("Describe this step…")
                        .min_lines(3)
                        .max_lines(12),
                    strip_edit(&s.photos, Some(id), 100.0),
                ]
                .spacing(6),
            );
        }
        g = g.push(
            flat_button(row![Icon::ListAdd.view(16.0), text("Add a step")].spacing(6))
                .on_press(Message::StepAdd),
        );
        g.into()
    }
}

fn parse(q: &str) -> Option<f64> {
    crate::app::parse_qty(q).ok().flatten()
}

fn ing_row(i: &IngRow, draft: bool) -> Element<'_, Message> {
    let id = i.id;
    let mut top = row![
        entry("Ingredient", i.name.as_str())
            .on_input(move |s| Message::IngName(id, s))
            .width(Length::Fill),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    if !draft {
        top = top.push(icon_button(Icon::Delete).on_press(Message::IngDelete(id)));
    }
    let mut bottom = row![
        entry("Qty", i.qty.as_str())
            .on_input(move |s| Message::IngQty(id, s))
            .width(80),
        entry("Unit", i.unit.as_str())
            .on_input(move |s| Message::IngUnit(id, s))
            .width(80),
        check_button("Optional", i.optional).on_toggle(move |on| Message::IngOptional(id, on)),
        space().width(Length::Fill),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    if draft {
        bottom = bottom.push(
            action_button("Add")
                .on_press_maybe((!i.name.trim().is_empty()).then_some(Message::IngAdd)),
        );
    } else {
        bottom = bottom
            .push(icon_button(Icon::GoUp).on_press(Message::IngMove(id, -1)))
            .push(icon_button(Icon::GoDown).on_press(Message::IngMove(id, 1)));
    }
    column![top, bottom].spacing(4).into()
}
