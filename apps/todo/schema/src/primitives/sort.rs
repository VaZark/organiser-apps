//! How tasks are ordered. Pinned tasks always come first; then the chosen key; and when two
//! tasks tie on it, the one with more subtasks first, then alphabetically.

use std::cmp::Ordering;
use std::collections::HashMap;

use super::task::Task;

/// What to order the tasks by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortKey {
    /// The person's own order (dragged, or the order they were added in).
    #[default]
    Manual,
    /// The most important first.
    Priority,
    /// The newest first.
    Created,
    /// The closest due date first, overdue ones included; tasks with no due date last.
    Due,
}

impl SortKey {
    pub const ALL: [SortKey; 4] = [Self::Manual, Self::Priority, Self::Created, Self::Due];

    pub fn label(self) -> &'static str {
        match self {
            Self::Manual => "Manual order",
            Self::Priority => "Priority",
            Self::Created => "Created",
            Self::Due => "Due date",
        }
    }
}

/// `a` before `b` by `key` alone: `Equal` is a tie, left to the tie-breakers.
fn by_key(key: SortKey, a: &Task, b: &Task) -> Ordering {
    match key {
        SortKey::Manual => Ordering::Equal,
        SortKey::Priority => b.priority.cmp(&a.priority),
        SortKey::Created => b.created.cmp(&a.created),
        // `false < true`: a task with a date before one without; then the earlier date.
        SortKey::Due => (a.due.is_none(), a.due).cmp(&(b.due.is_none(), b.due)),
    }
}

/// Whether `a` comes before `b`: pinned first, then `key`, then the more subtasks (`subs` counts
/// them by task id), then the title, ignoring case. [`SortKey::Manual`] has no tie-breakers: it
/// keeps whatever order the tasks are in.
pub fn ordering(key: SortKey, a: &Task, b: &Task, subs: &HashMap<i64, usize>) -> Ordering {
    let pinned = b.pinned.cmp(&a.pinned);
    if key == SortKey::Manual {
        return pinned;
    }
    let count = |t: &Task| subs.get(&t.id).copied().unwrap_or(0);
    pinned
        .then_with(|| by_key(key, a, b))
        .then_with(|| count(b).cmp(&count(a)))
        .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
}

/// Sort `tasks` by [`ordering`]. Stable, so under [`SortKey::Manual`] only the pinned move.
pub fn sort_tasks(tasks: &mut [Task], key: SortKey, subs: &HashMap<i64, usize>) {
    tasks.sort_by(|a, b| ordering(key, a, b, subs));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::priority::Priority;
    use organiser_engine::Item;

    fn t(id: i64, title: &str, priority: Priority, created: i64, due: Option<&str>) -> Task {
        let mut item = Item {
            id,
            kind: "task".into(),
            parent_id: None,
            title: title.into(),
            fields: Default::default(),
            tags: vec![],
            position: id,
            archived: false,
            created,
            updated: created,
        };
        item.fields.insert("priority".into(), priority.as_str().into());
        if let Some(d) = due {
            item.fields.insert("due".into(), d.into());
        }
        Task::from(&item)
    }

    fn titles(v: &[Task]) -> Vec<&str> {
        v.iter().map(|t| t.title.as_str()).collect()
    }

    #[test]
    fn priority_highest_first_then_more_subtasks_then_alphabetic() {
        let mut v = vec![
            t(1, "banana", Priority::High, 1, None),
            t(2, "Apple", Priority::High, 2, None),
            t(3, "cherry", Priority::High, 3, None),
            t(4, "zebra", Priority::Highest, 4, None),
            t(5, "low", Priority::Low, 5, None),
        ];
        // cherry has two subtasks, banana one, apple none.
        let subs = HashMap::from([(3, 2), (1, 1)]);
        sort_tasks(&mut v, SortKey::Priority, &subs);
        assert_eq!(titles(&v), ["zebra", "cherry", "banana", "Apple", "low"]);
    }

    #[test]
    fn due_closest_first_with_the_undated_last() {
        let mut v = vec![
            t(1, "none", Priority::Medium, 1, None),
            t(2, "later", Priority::Medium, 2, Some("2026-12-01")),
            t(3, "overdue", Priority::Medium, 3, Some("2026-01-01")),
            t(4, "soon b", Priority::Medium, 4, Some("2026-11-20")),
            t(5, "soon a", Priority::Medium, 5, Some("2026-11-20")),
        ];
        sort_tasks(&mut v, SortKey::Due, &HashMap::new());
        assert_eq!(titles(&v), ["overdue", "soon a", "soon b", "later", "none"]);
    }

    #[test]
    fn created_newest_first_and_pinned_always_first() {
        let mut v = vec![
            t(1, "old", Priority::Medium, 1, None),
            t(2, "new", Priority::Medium, 3, None),
            t(3, "mid", Priority::Medium, 2, None),
        ];
        sort_tasks(&mut v, SortKey::Created, &HashMap::new());
        assert_eq!(titles(&v), ["new", "mid", "old"]);
        v[2].pinned = true;
        sort_tasks(&mut v, SortKey::Created, &HashMap::new());
        assert_eq!(titles(&v), ["old", "new", "mid"]);
    }

    #[test]
    fn manual_keeps_the_order_but_for_the_pinned() {
        let mut v = vec![
            t(1, "b", Priority::Low, 1, None),
            t(2, "a", Priority::High, 2, None),
        ];
        sort_tasks(&mut v, SortKey::Manual, &HashMap::new());
        assert_eq!(titles(&v), ["b", "a"]);
    }
}
