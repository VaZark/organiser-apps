//! Tags edited in place: `tag2 ✕  tag3 ✕  +`. Each tag is a coloured chip with a ✕ to remove it.
//! The `+` at the end turns into a small entry in the same flow; Enter adds what was typed and
//! keeps adding, an empty Enter (or the app's Esc) stops. While adding, existing tags are offered
//! under the flow to pick instead of typing.

use iced::widget::{column, row};
use iced::{Alignment, Length};

use crate::Element;
use crate::components::chip::{pickable, removable};
use crate::iad::prelude::*;

/// How many suggestions to offer at once.
const SUGGESTIONS: usize = 6;
/// The id of the inline entry, which takes focus when it appears.
pub const TAG_ENTRY_ID: &str = "organiser.tag-entry";

/// Existing tags worth offering: not already on the item, containing what was typed.
pub fn suggestions<'a>(all: &'a [String], have: &[String], typed: &str) -> Vec<&'a str> {
    let typed = typed.trim().trim_start_matches(['#', '@']).to_lowercase();
    all.iter()
        .filter(|t| !have.iter().any(|h| h.eq_ignore_ascii_case(t)))
        .filter(|t| t.to_lowercase().contains(&typed))
        .map(String::as_str)
        .take(SUGGESTIONS)
        .collect()
}

/// `on_add` and `on_remove` build the app's message for a tag name, and `on_start` opens the
/// inline entry. Submitting the entry sends `on_add` with what is typed (possibly empty).
#[allow(clippy::too_many_arguments)]
pub fn tag_editor<'a, M: Clone + 'a>(
    tags: &'a [String],
    adding: bool,
    typed: &'a str,
    known: &'a [String],
    on_start: M,
    on_text: impl Fn(String) -> M + 'a,
    on_add: &dyn Fn(String) -> M,
    on_remove: &dyn Fn(String) -> M,
) -> Element<'a, M> {
    let mut flow = row(tags.iter().map(|t| removable(t, on_remove(t.clone()))))
        .spacing(6)
        .align_y(Alignment::Center);
    flow = if adding {
        flow.push(
            FocusScope::new(
                entry("New tag", typed)
                    .id(TAG_ENTRY_ID)
                    .on_input(on_text)
                    .on_submit(on_add(typed.to_string()))
                    .width(Length::Fixed(150.0)),
            )
            .trap(false)
            .initial_focus(TAG_ENTRY_ID),
        )
    } else {
        flow.push(tooltip(
            icon_button(Icon::ListAdd).on_press(on_start),
            "Add a tag",
            TooltipPosition::Bottom,
        ))
    };
    let mut col = column![flow.wrap()].spacing(8);
    if adding {
        let picks = suggestions(known, tags, typed);
        if !picks.is_empty() {
            col = col.push(
                row(picks
                    .into_iter()
                    .map(|t| pickable(t, on_add(t.to_string()))))
                .spacing(6)
                .wrap(),
            );
        }
    }
    col.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn offers_unused_tags_matching_what_is_typed() {
        let all = v(&["home", "work", "Homework", "travel"]);
        assert_eq!(
            suggestions(&all, &v(&["work"]), ""),
            ["home", "Homework", "travel"]
        );
        assert_eq!(suggestions(&all, &[], "#HOM"), ["home", "Homework"]);
        assert_eq!(suggestions(&all, &v(&["HOME"]), "ho"), ["Homework"]);
    }
}
