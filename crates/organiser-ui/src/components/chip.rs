//! Tags and badges as small rounded labels. A chip cannot wrap its text, so a long label is cut
//! with an ellipsis (the full text is in a tooltip) rather than spilling out of its background.

use iced::Color;

use crate::Element;
use crate::iad::Role;
use crate::iad::prelude::{Chip, ChipSize, TooltipPosition, tag, tooltip};
use crate::primitives::text::ellipsize;

/// The most characters a chip shows.
pub const MAX_CHARS: usize = 24;

/// A badge: `label` tinted by `role`, shortened if it is long.
pub fn pill<'a, M: 'a>(label: &str, role: Role) -> Element<'a, M> {
    let shown = ellipsize(label, MAX_CHARS);
    if shown == label {
        tag(shown, role)
    } else {
        tooltip(tag(shown, role), label.to_string(), TooltipPosition::Bottom)
    }
}

/// What a tag shows: `genre:Salsa` is `Salsa`.
fn shown(name: &str) -> &str {
    match name.split_once(':') {
        Some((_, v)) if !v.is_empty() => v,
        _ => name,
    }
}

/// The colours the app's tags have stored, by lowercase name. Every chip asks here, so telling
/// it once (see [`set_tag_colors`]) colours tags everywhere without passing colours around.
static TAG_COLORS: std::sync::RwLock<Option<std::collections::HashMap<String, Color>>> =
    std::sync::RwLock::new(None);

/// Record the colour of each tag that has one. Call it after (re)loading the tags.
pub fn set_tag_colors(tags: &[organiser_engine::Tag]) {
    let map = tags
        .iter()
        .filter_map(|t| {
            let c = crate::components::dot::color_from_hex(t.color.as_deref()?)?;
            Some((t.name.to_lowercase(), c))
        })
        .collect();
    if let Ok(mut w) = TAG_COLORS.write() {
        *w = Some(map);
    }
}

/// A tag's colour: the one stored for it, else one made from its name (the same name, ignoring
/// case, is always the same colour).
pub fn color_of(name: &str) -> Color {
    let key = name.to_lowercase();
    TAG_COLORS
        .read()
        .ok()
        .and_then(|m| m.as_ref().and_then(|m| m.get(&key).copied()))
        .unwrap_or_else(|| crate::iad::color::from_seed(key))
}

fn colored<'a, M: Clone + 'a>(name: &str) -> Chip<'a, M> {
    Chip::new(ellipsize(shown(name), MAX_CHARS))
        .color(color_of(name))
        .size(ChipSize::Small)
}

/// A tag, in its own colour.
pub fn chip<'a, M: Clone + 'a>(name: &str) -> Element<'a, M> {
    colored(name).into()
}

/// A tag as a hashtag, `# name`, in its own colour: how a card shows its tags.
pub fn hashtag<'a, M: Clone + 'a>(name: &str) -> Element<'a, M> {
    Chip::new(format!("# {}", ellipsize(shown(name), MAX_CHARS)))
        .color(color_of(name))
        .size(ChipSize::Small)
        .into()
}

/// A tag with a ✕ that sends `on_remove`.
pub fn removable<'a, M: Clone + 'a>(name: &str, on_remove: M) -> Element<'a, M> {
    colored(name).on_remove(on_remove).into()
}

/// A tag that can be pressed, to pick it.
pub fn pickable<'a, M: Clone + 'a>(name: &str, on_press: M) -> Element<'a, M> {
    colored(name).on_press(on_press).into()
}

/// `+3`: how many tags a crowded row left out.
pub fn more<'a, M: 'a>(n: usize) -> Element<'a, M> {
    tag(format!("+{n}"), Role::Neutral)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(name: &str, color: Option<&str>) -> organiser_engine::Tag {
        organiser_engine::Tag {
            name: name.into(),
            key: None,
            value: name.into(),
            parent: None,
            icon: None,
            color: color.map(String::from),
            counts: vec![],
        }
    }

    #[test]
    fn a_stored_colour_wins_and_other_tags_keep_their_name_colour() {
        let fallback = color_of("unlisted-tag");
        set_tag_colors(&[tag("Work", Some("#ff0000")), tag("plain", None)]);
        assert_eq!(
            color_of("work"),
            Color::from_rgb8(255, 0, 0),
            "case does not matter"
        );
        assert_eq!(color_of("WORK"), Color::from_rgb8(255, 0, 0));
        assert_eq!(color_of("unlisted-tag"), fallback);
        // `plain` has no stored colour, so it falls back like any unknown tag.
        assert_eq!(color_of("plain"), crate::iad::color::from_seed("plain"));
    }
}
