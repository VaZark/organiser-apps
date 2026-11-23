//! Turning a flat, ordered list of items into a forest and back into visible rows. Generic over
//! the item type, so every app with nested items shares it.

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub struct Node<T> {
    pub value: T,
    pub children: Vec<Node<T>>,
}

/// Build a forest keeping the input order. An item whose parent is not in `items` (filtered
/// out, say) becomes a root, so nothing disappears.
pub fn build<T>(
    items: Vec<T>,
    id: impl Fn(&T) -> i64,
    parent: impl Fn(&T) -> Option<i64>,
) -> Vec<Node<T>> {
    let ids: HashSet<i64> = items.iter().map(&id).collect();
    let mut by_parent: HashMap<Option<i64>, Vec<T>> = HashMap::new();
    for it in items {
        let p = parent(&it).filter(|p| ids.contains(p));
        by_parent.entry(p).or_default().push(it);
    }
    fn grow<T>(
        level: Vec<T>,
        by_parent: &mut HashMap<Option<i64>, Vec<T>>,
        id: &impl Fn(&T) -> i64,
    ) -> Vec<Node<T>> {
        level
            .into_iter()
            .map(|v| {
                let kids = by_parent.remove(&Some(id(&v))).unwrap_or_default();
                Node {
                    children: grow(kids, by_parent, id),
                    value: v,
                }
            })
            .collect()
    }
    let roots = by_parent.remove(&None).unwrap_or_default();
    grow(roots, &mut by_parent, &id)
}

/// One visible row: how deep it is, whether it has children, and its value.
#[derive(Debug, Clone, PartialEq)]
pub struct Row<'a, T> {
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
    pub value: &'a T,
}

/// The rows to show: every root, and the children of nodes whose id is in `expanded`.
pub fn flatten<'a, T>(
    nodes: &'a [Node<T>],
    expanded: &HashSet<i64>,
    id: &impl Fn(&T) -> i64,
) -> Vec<Row<'a, T>> {
    fn walk<'a, T>(
        nodes: &'a [Node<T>],
        depth: usize,
        expanded: &HashSet<i64>,
        id: &impl Fn(&T) -> i64,
        out: &mut Vec<Row<'a, T>>,
    ) {
        for n in nodes {
            let open = expanded.contains(&id(&n.value));
            out.push(Row {
                depth,
                has_children: !n.children.is_empty(),
                expanded: open,
                value: &n.value,
            });
            if open {
                walk(&n.children, depth + 1, expanded, id, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(nodes, 0, expanded, id, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // (id, parent)
    type T = (i64, Option<i64>);

    fn forest() -> Vec<Node<T>> {
        build(
            vec![
                (1, None),
                (2, Some(1)),
                (3, Some(2)),
                (4, None),
                (5, Some(99)),
            ],
            |t| t.0,
            |t| t.1,
        )
    }

    #[test]
    fn orphans_become_roots_and_order_is_kept() {
        let f = forest();
        assert_eq!(f.iter().map(|n| n.value.0).collect::<Vec<_>>(), [1, 4, 5]);
        assert_eq!(f[0].children[0].children[0].value.0, 3);
    }

    #[test]
    fn flatten_follows_expansion() {
        let f = forest();
        let ids = |e: &[i64]| {
            let set: HashSet<i64> = e.iter().copied().collect();
            flatten(&f, &set, &|t: &T| t.0)
                .iter()
                .map(|r| (r.value.0, r.depth))
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&[]), [(1, 0), (4, 0), (5, 0)]);
        assert_eq!(ids(&[1]), [(1, 0), (2, 1), (4, 0), (5, 0)]);
        assert_eq!(ids(&[1, 2]), [(1, 0), (2, 1), (3, 2), (4, 0), (5, 0)]);
    }
}
