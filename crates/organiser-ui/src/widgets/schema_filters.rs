//! The filter controls an app's schema asks for: one per filterable field (see
//! [`organiser_engine::filterable`]), by the field's type. Chips for a choice among values
//! (an enum, a reference to a status or an epic), a check for a flag, "at least" stars for a
//! rating. Dates and plain numbers are not offered yet.
//!
//! The widget holds no state: it shows a [`Filters`] and sends back the changed one.

use iced::widget::column;
use organiser_engine::{Field, FieldType, Filters, Kind, filterable};
use serde_json::Value;

use crate::Element;
use crate::iad::prelude::*;

/// One value a reference field can take: an epic, a status.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub value: Value,
    pub label: String,
    pub color: Option<iced::Color>,
}

/// `status_text` → `Status text`: a field name as a person reads it.
pub fn label_of(name: &str) -> String {
    let s = name.replace('_', " ");
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// `refs` gives the choices of a [`FieldType::Reference`] field, which only the app knows.
pub fn schema_filters<'a, M: Clone + 'a>(
    kind: &'static Kind,
    state: &Filters,
    refs: &dyn Fn(&Field) -> Vec<Choice>,
    on_change: impl Fn(Filters) -> M + Clone + 'a,
) -> Element<'a, M> {
    let mut col = column![].spacing(12);
    for f in filterable(kind) {
        let name = f.name;
        let st = state.clone();
        let oc = on_change.clone();
        let control: Option<Element<'a, M>> = match f.ty {
            FieldType::Enum(opts) => {
                let choices: Vec<Choice> = opts
                    .iter()
                    .map(|o| Choice {
                        value: Value::from(*o),
                        label: label_of(o),
                        color: None,
                    })
                    .collect();
                Some(chips(name, choices, &st, oc))
            }
            FieldType::Reference(_) => {
                let choices = refs(f);
                (!choices.is_empty()).then(|| chips(name, choices, &st, oc))
            }
            FieldType::Bool => {
                let on = st.any_of(name).contains(&Value::Bool(true));
                Some(
                    EntryRow::switch(label_of(name), on, move |_| {
                        let mut s = st.clone();
                        s.toggle(name, Value::Bool(true));
                        oc(s)
                    })
                    .flat(true)
                    .into(),
                )
            }
            FieldType::Rating => Some(stars(name, &st, oc)),
            _ => None,
        };
        if let Some(c) = control {
            // Each control is a row with its label inside it; a flag is a switch row.
            col = col.push(match f.ty {
                FieldType::Bool => c,
                _ => EntryRow::new(label_of(name), EntryInput::Custom(c))
                    .flat(true)
                    .into(),
            });
        }
    }
    col.into()
}

fn chips<'a, M: Clone + 'a>(
    name: &'static str,
    choices: Vec<Choice>,
    state: &Filters,
    on_change: impl Fn(Filters) -> M + 'a,
) -> Element<'a, M> {
    let selected: Vec<usize> = choices
        .iter()
        .enumerate()
        .filter(|(_, c)| state.any_of(name).contains(&c.value))
        .map(|(i, _)| i)
        .collect();
    let options: Vec<ChipOption<usize>> = choices
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let o = ChipOption::new(i, c.label.clone());
            match c.color {
                Some(col) => o.color(col),
                None => o,
            }
        })
        .collect();
    let values: Vec<Value> = choices.into_iter().map(|c| c.value).collect();
    let state = state.clone();
    ChipGroup::new(options, selected, move |i: usize| {
        let mut s = state.clone();
        s.toggle(name, values[i].clone());
        on_change(s)
    })
    .into()
}

fn stars<'a, M: Clone + 'a>(
    name: &'static str,
    state: &Filters,
    on_change: impl Fn(Filters) -> M + 'a,
) -> Element<'a, M> {
    let min = match state.get(name) {
        Some(organiser_engine::FilterValue::Range { from: Some(v), .. }) => {
            v.as_u64().map(|n| n as u8)
        }
        _ => None,
    };
    let options: Vec<ChipOption<u8>> = (1..=5u8)
        .map(|n| ChipOption::new(n, format!("{n}★ and up")))
        .collect();
    let state = state.clone();
    ChipGroup::new(
        options,
        min.into_iter().collect::<Vec<_>>(),
        move |n: u8| {
            let mut s = state.clone();
            // Pressing the chosen minimum again clears it.
            let next = (min != Some(n)).then(|| Value::from(n));
            s.set_range(name, next, None);
            on_change(s)
        },
    )
    .into()
}

/// A chip for each filter that is on, each removing itself when pressed: how a page shows what
/// narrows it. `refs` is as for [`schema_filters`], to name a reference by its label.
pub fn active_filters<'a, M: Clone + 'a>(
    kind: &'static Kind,
    state: &Filters,
    refs: &dyn Fn(&Field) -> Vec<Choice>,
    on_change: impl Fn(Filters) -> M + Clone + 'a,
) -> Vec<Element<'a, M>> {
    let mut out = Vec::new();
    let chip = |label: String, msg: M| -> Element<'a, M> {
        flat_button(
            iced::widget::row![text(label), Icon::WindowClose.view(12.0)]
                .spacing(6)
                .align_y(iced::Alignment::Center),
        )
        .on_press(msg)
        .into()
    };
    for f in filterable(kind) {
        match state.get(f.name) {
            Some(organiser_engine::FilterValue::AnyOf(values)) => {
                let known = refs(f);
                for v in values {
                    let label = match f.ty {
                        FieldType::Bool => label_of(f.name),
                        FieldType::Enum(_) => v.as_str().map(label_of).unwrap_or_default(),
                        _ => known
                            .iter()
                            .find(|c| &c.value == v)
                            .map_or_else(|| v.to_string(), |c| c.label.clone()),
                    };
                    let mut s = state.clone();
                    s.toggle(f.name, v.clone());
                    out.push(chip(label, on_change(s)));
                }
            }
            Some(organiser_engine::FilterValue::Range { from, to }) => {
                let show = |v: &Option<Value>| {
                    v.as_ref()
                        .map(|v| v.to_string().trim_matches('"').to_string())
                };
                let label = match (show(from), show(to)) {
                    (Some(a), Some(b)) => format!("{}: {a} to {b}", label_of(f.name)),
                    (Some(a), None) => format!("{}: {a} and up", label_of(f.name)),
                    (None, Some(b)) => format!("{}: up to {b}", label_of(f.name)),
                    (None, None) => continue,
                };
                let mut s = state.clone();
                s.remove(f.name);
                out.push(chip(label, on_change(s)));
            }
            None => {}
        }
    }
    out
}
