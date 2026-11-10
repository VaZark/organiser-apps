//! The tag vocabulary in tree order: a parent before its children, siblings by name.

use organiser_engine::Tag;

/// Depth of `tag` in the tag tree (roots are 0). Cycles cannot exist, but stop anyway.
fn depth(tag: &Tag, all: &[Tag]) -> usize {
    let mut d = 0;
    let mut up = tag.parent.as_deref();
    while let Some(p) = up {
        d += 1;
        if d > 16 {
            break;
        }
        up = all
            .iter()
            .find(|t| t.name == p)
            .and_then(|t| t.parent.as_deref());
    }
    d
}

/// Tags in tree order (a parent before its children, siblings by name) with their depth.
pub fn ordered(tags: &[Tag]) -> Vec<(usize, &Tag)> {
    fn walk<'a>(tags: &'a [Tag], parent: Option<&str>, out: &mut Vec<(usize, &'a Tag)>) {
        for t in tags.iter().filter(|t| t.parent.as_deref() == parent) {
            out.push((depth(t, tags), t));
            walk(tags, Some(&t.name), out);
        }
    }
    let mut out = Vec::new();
    walk(tags, None, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(name: &str, parent: Option<&str>) -> Tag {
        Tag {
            name: name.into(),
            key: None,
            value: name.into(),
            parent: parent.map(Into::into),
            icon: None,
            color: None,
            counts: Vec::new(),
        }
    }

    #[test]
    fn parents_come_before_children() {
        let tags = vec![
            tag("b", None),
            tag("a", Some("b")),
            tag("c", None),
            tag("d", Some("a")),
        ];
        let got: Vec<_> = ordered(&tags)
            .iter()
            .map(|(d, t)| (*d, t.name.as_str()))
            .collect();
        assert_eq!(got, [(0, "b"), (1, "a"), (2, "d"), (0, "c")]);
    }
}
