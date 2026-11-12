//! Items in groups, for a list shown in sections: one section per folder or epic, in the order the
//! person arranged them, and the items that belong to none gathered last, as "unassigned".

/// `items` dealt into the groups named in `order` (by the key `key` gives each), each group with
/// its items in the order they came in, and left out when it has none. The items whose key is
/// `None`, or is not in `order` (a group that no longer exists), come last under `None`.
pub fn group_by<T>(
    items: Vec<T>,
    key: impl Fn(&T) -> Option<i64>,
    order: &[i64],
) -> Vec<(Option<i64>, Vec<T>)> {
    let mut groups: Vec<(Option<i64>, Vec<T>)> = order.iter().map(|id| (Some(*id), Vec::new())).collect();
    let mut unassigned = Vec::new();
    for item in items {
        match key(&item).and_then(|k| groups.iter_mut().find(|(id, _)| *id == Some(k))) {
            Some((_, v)) => v.push(item),
            None => unassigned.push(item),
        }
    }
    groups.retain(|(_, v)| !v.is_empty());
    if !unassigned.is_empty() {
        groups.push((None, unassigned));
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_follow_the_given_order_and_the_unassigned_come_last() {
        let items = vec![("a", Some(2)), ("b", None), ("c", Some(1)), ("d", Some(2)), ("e", Some(9))];
        let g = group_by(items, |i| i.1, &[1, 2, 3]);
        let names: Vec<(Option<i64>, Vec<&str>)> = g
            .iter()
            .map(|(k, v)| (*k, v.iter().map(|i| i.0).collect()))
            .collect();
        // Group 3 has nothing and is left out; "e" is in a group that does not exist.
        assert_eq!(
            names,
            vec![
                (Some(1), vec!["c"]),
                (Some(2), vec!["a", "d"]),
                (None, vec!["b", "e"]),
            ]
        );
    }

    #[test]
    fn nothing_grouped_is_nothing() {
        assert!(group_by(Vec::<i32>::new(), |_| None, &[1]).is_empty());
    }
}
