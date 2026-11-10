//! Which nodes of a tree are folded. Everything starts open, so a new item shows its subtasks;
//! the user folds what they do not want to see.

use std::collections::HashSet;

use organiser_engine::{Node, Row, flatten_tree};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Folds {
    folded: HashSet<i64>,
}

impl Folds {
    pub fn toggle(&mut self, id: i64) {
        if !self.folded.remove(&id) {
            self.folded.insert(id);
        }
    }

    pub fn is_folded(&self, id: i64) -> bool {
        self.folded.contains(&id)
    }

    /// The visible rows of a forest.
    pub fn rows<'a, T>(&self, nodes: &'a [Node<T>], id: impl Fn(&T) -> i64) -> Vec<Row<'a, T>> {
        fn collect<T>(nodes: &[Node<T>], id: &impl Fn(&T) -> i64, out: &mut HashSet<i64>) {
            for n in nodes {
                out.insert(id(&n.value));
                collect(&n.children, id, out);
            }
        }
        let mut open = HashSet::new();
        collect(nodes, &id, &mut open);
        for f in &self.folded {
            open.remove(f);
        }
        flatten_tree(nodes, &open, &id)
    }
}

/// Every node of a forest as an equal row, depth first: a parent is followed by its children,
/// but nothing is indented, folded or marked as having children.
pub fn flat_rows<T>(nodes: &[Node<T>]) -> Vec<Row<'_, T>> {
    fn walk<'a, T>(nodes: &'a [Node<T>], out: &mut Vec<Row<'a, T>>) {
        for n in nodes {
            out.push(Row {
                depth: 0,
                has_children: false,
                expanded: false,
                value: &n.value,
            });
            walk(&n.children, out);
        }
    }
    let mut out = Vec::new();
    walk(nodes, &mut out);
    out
}

/// The items as equal rows, in the order given.
pub fn plain_rows<T>(items: &[T]) -> Vec<Row<'_, T>> {
    items
        .iter()
        .map(|value| Row {
            depth: 0,
            has_children: false,
            expanded: false,
            value,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use organiser_engine::build_tree;

    #[test]
    fn open_by_default_and_folds_hide_children() {
        let forest = build_tree(
            vec![(1, None), (2, Some(1)), (3, Some(2))],
            |t: &(i64, Option<i64>)| t.0,
            |t| t.1,
        );
        let mut f = Folds::default();
        let ids = |f: &Folds| {
            f.rows(&forest, |t| t.0)
                .iter()
                .map(|r| r.value.0)
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&f), [1, 2, 3]);
        f.toggle(2);
        assert_eq!(ids(&f), [1, 2]);
        f.toggle(1);
        assert_eq!(ids(&f), [1]);
        f.toggle(1);
        f.toggle(2);
        assert_eq!(ids(&f), [1, 2, 3]);
    }

    #[test]
    fn flat_rows_keep_every_node_at_one_level_in_tree_order() {
        let forest = build_tree(
            vec![(1, None), (2, Some(1)), (3, Some(2)), (4, None)],
            |t: &(i64, Option<i64>)| t.0,
            |t| t.1,
        );
        let rows = flat_rows(&forest);
        assert_eq!(
            rows.iter().map(|r| r.value.0).collect::<Vec<_>>(),
            [1, 2, 3, 4]
        );
        assert!(
            rows.iter()
                .all(|r| r.depth == 0 && !r.has_children && !r.expanded)
        );
    }

    #[test]
    fn plain_rows_keep_the_given_order_at_one_level() {
        let rows = plain_rows(&[3, 1, 2]);
        assert_eq!(rows.iter().map(|r| *r.value).collect::<Vec<_>>(), [3, 1, 2]);
        assert!(rows.iter().all(|r| r.depth == 0 && !r.has_children));
    }
}
