//! Tag groups as a read-only line, each tag in its own colour: groups are alternatives and the
//! tags in one go together. A thin cover over iad's `TagGroupList` that looks the colours up.

use crate::Element;
use crate::components::chip::color_of;
use crate::iad::prelude::{TagGroupList, TagItem};

/// `groups` are tag names. `on_remove` gets the group and tag index of a chip whose ✕ was pressed.
pub fn tag_groups<'a, M: Clone + 'a>(
    groups: &[Vec<String>],
    on_remove: impl Fn(usize, usize) -> M + 'a,
) -> Element<'a, M> {
    let items = groups
        .iter()
        .map(|g| {
            g.iter()
                .map(|name| TagItem::new(shown(name)).color(color_of(name)))
                .collect()
        })
        .collect();
    TagGroupList::new(items).on_remove(on_remove).into()
}

/// What a tag shows: `genre:Salsa` is `Salsa`.
fn shown(name: &str) -> String {
    match name.split_once(':') {
        Some((_, v)) if !v.is_empty() => v.to_string(),
        _ => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::shown;

    #[test]
    fn a_keyed_tag_shows_its_value() {
        assert_eq!(shown("genre:Salsa"), "Salsa");
        assert_eq!(shown("work"), "work");
        assert_eq!(shown("odd:"), "odd:");
    }
}
