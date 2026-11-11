//! The tags part of a filter: iad's `TagFilter` with several alternative groups (an item needs
//! every tag of at least one group), each tag as a chip in its own colour with how many things
//! carry it. What picking means is the row's description, underneath. Tags are managed
//! (coloured, grouped) in their own dialog, from the header.
//!
//! The app owns the picked [`TagGroups`] and the search text, and turns them into the engine's
//! filter with [`engine_filter`](crate::components::tag_query::engine_filter).

use organiser_engine::Tag;

use crate::Element;
use crate::components::chip::color_of;
use crate::iad::prelude::*;
use crate::iad::primitives::tag_groups::{GroupEvent, TagGroups};
use crate::primitives::text::ellipsize;
use crate::widgets::tag_tree;

pub fn tag_filter<'a, M: Clone + 'a>(
    tags: &[Tag],
    groups: &'a TagGroups<String>,
    query: &'a str,
    hint: &'a str,
    on_groups: impl Fn(GroupEvent<String>) -> M + 'a,
    on_query: impl Fn(String) -> M + 'a,
) -> Element<'a, M> {
    let options: Vec<ChipOption<String>> = tag_tree::ordered(tags)
        .into_iter()
        .map(|(_, t)| {
            ChipOption::new(
                t.name.clone(),
                format!("{}  ({})", ellipsize(&t.name, 28), t.count("item")),
            )
            .color(color_of(&t.name))
        })
        .collect();
    if options.is_empty() {
        return EntryRow::new("Tags", EntryInput::Custom(caption("No tags yet.").into()))
            .description(hint)
            .flat(true)
            .into();
    }
    TagFilter::new(groups, options, query, on_groups, on_query)
        .multiple(true)
        .label("Tags")
        .description(hint)
        .placeholder("Find a tag")
        .flat(true)
        .into()
}
