//! What the tag filter's groups ask of the engine. The groups are alternatives: an item needs every
//! tag of at least one group (`a` and `b`, or `c`).

use organiser_engine::TagFilter;

use crate::iad::primitives::tag_groups::TagGroups;

/// The engine filter for the picked groups. Empty groups restrict nothing, and no tags at all
/// is no filter.
pub fn engine_filter(groups: &TagGroups<String>) -> TagFilter {
    let leaves: Vec<TagFilter> = groups
        .groups()
        .iter()
        .filter(|g| !g.is_empty())
        .map(|g| TagFilter {
            all: g.clone(),
            ..TagFilter::default()
        })
        .collect();
    if leaves.is_empty() {
        TagFilter::default()
    } else {
        TagFilter {
            blocks: vec![leaves],
            ..TagFilter::default()
        }
    }
}

/// How many tags are picked, over all groups.
pub fn count(groups: &TagGroups<String>) -> usize {
    groups.groups().iter().map(Vec::len).sum()
}

/// The groups with the tags `exists` rejects taken out (a tag that was deleted no longer
/// filters), keeping the group being edited where it was.
pub fn retain_existing(
    groups: &TagGroups<String>,
    exists: impl Fn(&str) -> bool,
) -> TagGroups<String> {
    let kept: Vec<Vec<String>> = groups
        .groups()
        .iter()
        .map(|g| g.iter().filter(|t| exists(t)).cloned().collect())
        .collect();
    if kept == groups.groups() {
        return groups.clone();
    }
    TagGroups::from_parts(kept, groups.active())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn groups(g: &[&[&str]]) -> TagGroups<String> {
        TagGroups::from_parts(
            g.iter()
                .map(|g| g.iter().map(|t| t.to_string()).collect())
                .collect(),
            0,
        )
    }

    #[test]
    fn no_tags_is_no_filter() {
        assert_eq!(engine_filter(&TagGroups::new()), TagFilter::default());
        assert_eq!(engine_filter(&groups(&[&[], &[]])), TagFilter::default());
    }

    #[test]
    fn groups_are_alternatives_and_the_tags_in_one_are_all_needed() {
        let f = engine_filter(&groups(&[&["a", "b"], &["c"]]));
        assert_eq!(f.blocks.len(), 1, "one block of alternatives");
        let alternatives = &f.blocks[0];
        assert_eq!(alternatives.len(), 2);
        assert_eq!(alternatives[0].all, ["a", "b"]);
        assert_eq!(alternatives[1].all, ["c"]);
        assert!(f.all.is_empty() && f.any.is_empty());
    }

    #[test]
    fn counting_adds_up_every_group() {
        assert_eq!(count(&groups(&[&["a", "b"], &["c"]])), 3);
        assert_eq!(count(&TagGroups::new()), 0);
    }

    #[test]
    fn deleted_tags_leave_the_groups_and_untouched_groups_are_returned_as_they_were() {
        let g = groups(&[&["a", "b"], &["c"]]);
        assert_eq!(retain_existing(&g, |_| true), g);
        let pruned = retain_existing(&g, |t| t != "b");
        assert_eq!(pruned.groups()[0], ["a"]);
        assert_eq!(pruned.groups()[1], ["c"]);
    }
}
