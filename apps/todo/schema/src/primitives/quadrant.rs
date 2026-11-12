//! The Eisenhower matrix: which of four boxes a task is in, by how much it matters and how soon
//! it is due. A task is important when its priority is high or highest, and urgent when it is due
//! within the week (or already overdue). A task with no due date is not urgent.

use chrono::NaiveDate;

use std::cmp::Ordering;
use std::collections::HashMap;

use super::priority::Priority;
use super::task::Task;

/// How many days ahead a due date makes a task urgent.
pub const URGENT_DAYS: i64 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Quadrant {
    /// Urgent and important: do it now.
    DoFirst,
    /// Important, not urgent: plan when to do it.
    Schedule,
    /// Urgent, not important: pass it on, or keep it short.
    Delegate,
    /// Neither: it can wait, and may not need doing at all.
    Backlog,
}

impl Quadrant {
    /// In reading order: top left, top right, bottom left, bottom right.
    pub const ALL: [Quadrant; 4] = [Self::DoFirst, Self::Schedule, Self::Delegate, Self::Backlog];

    pub fn title(self) -> &'static str {
        match self {
            Self::DoFirst => "Do first",
            Self::Schedule => "Schedule",
            Self::Delegate => "Delegate",
            Self::Backlog => "Backlog",
        }
    }

    /// Whether tasks here are urgent.
    pub fn urgent(self) -> bool {
        matches!(self, Self::DoFirst | Self::Delegate)
    }

    /// Whether tasks here are important.
    pub fn important(self) -> bool {
        matches!(self, Self::DoFirst | Self::Schedule)
    }
}

/// Whether a priority counts as important: high or highest.
pub fn is_important(p: Priority) -> bool {
    p >= Priority::High
}

/// Whether a due date makes a task urgent: within [`URGENT_DAYS`] days, or overdue.
pub fn is_urgent(due: Option<NaiveDate>, today: NaiveDate) -> bool {
    due.is_some_and(|d| (d - today).num_days() <= URGENT_DAYS)
}

/// The box a task with this priority and due date is in.
pub fn classify(priority: Priority, due: Option<NaiveDate>, today: NaiveDate) -> Quadrant {
    match (is_urgent(due, today), is_important(priority)) {
        (true, true) => Quadrant::DoFirst,
        (false, true) => Quadrant::Schedule,
        (true, false) => Quadrant::Delegate,
        (false, false) => Quadrant::Backlog,
    }
}

/// The order of triage inside a box: pinned tasks first, then the more important, then the
/// closer due date (a task with no date after those that have one), then the more subtasks
/// (`subs` counts them by task id), then the title, ignoring case.
pub fn triage_order(a: &Task, b: &Task, subs: &HashMap<i64, usize>) -> Ordering {
    let count = |t: &Task| subs.get(&t.id).copied().unwrap_or(0);
    b.pinned
        .cmp(&a.pinned)
        .then_with(|| b.priority.cmp(&a.priority))
        .then_with(|| (a.due.is_none(), a.due).cmp(&(b.due.is_none(), b.due)))
        .then_with(|| count(b).cmp(&count(a)))
        .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }
    const TODAY: &str = "2026-11-10";

    #[test]
    fn important_and_urgent_decide_the_box() {
        let today = d(TODAY);
        let soon = Some(d("2026-11-14"));
        let later = Some(d("2026-12-25"));
        assert_eq!(classify(Priority::High, soon, today), Quadrant::DoFirst);
        assert_eq!(classify(Priority::Highest, later, today), Quadrant::Schedule);
        assert_eq!(classify(Priority::Low, soon, today), Quadrant::Delegate);
        assert_eq!(classify(Priority::Medium, later, today), Quadrant::Backlog);
    }

    #[test]
    fn no_date_is_not_urgent_and_overdue_is() {
        let today = d(TODAY);
        assert!(!is_urgent(None, today));
        assert!(is_urgent(Some(d("2026-10-01")), today), "overdue");
        assert!(is_urgent(Some(d("2026-11-17")), today), "a week away is the edge");
        assert!(!is_urgent(Some(d("2026-11-18")), today));
    }

    #[test]
    fn medium_is_not_important() {
        assert!(!is_important(Priority::Medium) && is_important(Priority::High));
    }

    fn task(id: i64, title: &str, priority: Priority, due: Option<&str>) -> Task {
        use organiser_engine::Item;
        let mut item = Item {
            id,
            kind: "task".into(),
            parent_id: None,
            title: title.into(),
            fields: Default::default(),
            tags: vec![],
            position: id,
            archived: false,
            created: id,
            updated: id,
        };
        item.fields.insert("priority".into(), priority.as_str().into());
        if let Some(d) = due {
            item.fields.insert("due".into(), d.into());
        }
        Task::from(&item)
    }

    #[test]
    fn triage_puts_the_important_first_then_the_closer_date_then_more_subtasks_then_the_title() {
        let mut v = vec![
            task(1, "no date", Priority::High, None),
            task(2, "later", Priority::High, Some("2026-11-20")),
            task(3, "sooner", Priority::High, Some("2026-11-12")),
            task(4, "top", Priority::Highest, Some("2026-12-30")),
            task(5, "b tie", Priority::High, Some("2026-11-20")),
            task(6, "a tie", Priority::High, Some("2026-11-20")),
            task(7, "crowded", Priority::High, Some("2026-11-20")),
        ];
        let subs = HashMap::from([(7, 3)]);
        v.sort_by(|a, b| triage_order(a, b, &subs));
        let names: Vec<&str> = v.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(
            names,
            ["top", "sooner", "crowded", "a tie", "b tie", "later", "no date"]
        );
        v[6].pinned = true;
        v.sort_by(|a, b| triage_order(a, b, &subs));
        assert_eq!(v[0].title, "no date", "pinned first");
    }
}
