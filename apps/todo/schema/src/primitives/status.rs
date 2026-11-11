//! Statuses a task can be in. They are the person's own: a name, a colour and whether the status
//! means finished. [`Statuses`] is the ordered set and answers every question about them, so no
//! other code needs to know which statuses exist.

use organiser_engine::Item;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One status. Order among statuses is their position (the kanban columns, left to right).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    pub id: i64,
    pub name: String,
    /// `#rrggbb`: the status dot, and the tint of a card in this status.
    pub color: Option<String>,
    /// Finished: done, or decided against. Closed tasks are hidden unless asked for, and
    /// closing a task closes its subtasks.
    pub closed: bool,
    pub position: i64,
}

impl From<&Item> for Status {
    fn from(i: &Item) -> Self {
        Self {
            id: i.id,
            name: i.title.clone(),
            color: i
                .fields
                .get("color")
                .and_then(Value::as_str)
                .filter(|c| is_hex(c))
                .map(String::from),
            closed: i
                .fields
                .get("closed")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            position: i.position,
        }
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

/// Whether `s` is `#rrggbb`.
pub fn is_hex(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// What a fresh database starts with: name, colour, closed, and the name the status had when it
/// was a fixed list (so older data can be moved over).
pub const DEFAULTS: [(&str, &str, bool, &str); 4] = [
    ("To do", "#8e99a8", false, "open"),
    ("In progress", "#6aa5e8", false, "in_progress"),
    ("Done", "#7fc97f", true, "done"),
    ("Won't do", "#e0787a", true, "dismissed"),
];

/// The statuses in order. A task whose status is missing or unknown counts as being in the first
/// open status, so nothing is ever without one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Statuses(Vec<Status>);

impl Statuses {
    /// `list` in position order.
    pub fn new(mut list: Vec<Status>) -> Self {
        list.sort_by_key(|s| (s.position, s.id));
        Self(list)
    }

    pub fn all(&self) -> &[Status] {
        &self.0
    }

    pub fn get(&self, id: i64) -> Option<&Status> {
        self.0.iter().find(|s| s.id == id)
    }

    /// The first status that is not finished: where a new task starts.
    pub fn first_open(&self) -> Option<&Status> {
        self.0.iter().find(|s| !s.closed)
    }

    /// The first finished status: what "done" means.
    pub fn first_closed(&self) -> Option<&Status> {
        self.0.iter().find(|s| s.closed)
    }

    /// The status a task is really in: its own, or the first open one when it has none or its
    /// status is gone.
    pub fn resolve(&self, id: Option<i64>) -> Option<&Status> {
        id.and_then(|i| self.get(i)).or_else(|| self.first_open())
    }

    pub fn is_closed(&self, id: Option<i64>) -> bool {
        self.resolve(id).is_some_and(|s| s.closed)
    }

    pub fn is_open(&self, id: Option<i64>) -> bool {
        !self.is_closed(id)
    }

    /// A status by name, ignoring case.
    pub fn by_name(&self, name: &str) -> Option<&Status> {
        self.0
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name.trim()))
    }

    pub fn open(&self) -> impl Iterator<Item = &Status> {
        self.0.iter().filter(|s| !s.closed)
    }

    pub fn closed(&self) -> impl Iterator<Item = &Status> {
        self.0.iter().filter(|s| s.closed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(id: i64, name: &str, closed: bool, position: i64) -> Status {
        Status {
            id,
            name: name.into(),
            color: None,
            closed,
            position,
        }
    }

    fn set() -> Statuses {
        // Given out of order: position decides.
        Statuses::new(vec![
            st(3, "Done", true, 2),
            st(1, "To do", false, 0),
            st(2, "Doing", false, 1),
        ])
    }

    #[test]
    fn statuses_are_kept_in_position_order() {
        let set = set();
        let names: Vec<_> = set.all().iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["To do", "Doing", "Done"]);
    }

    #[test]
    fn new_work_starts_in_the_first_open_status_and_done_is_the_first_closed() {
        let s = set();
        assert_eq!(s.first_open().map(|s| s.id), Some(1));
        assert_eq!(s.first_closed().map(|s| s.id), Some(3));
    }

    #[test]
    fn a_missing_or_unknown_status_counts_as_the_first_open_one() {
        let s = set();
        assert_eq!(s.resolve(None).map(|s| s.id), Some(1));
        assert_eq!(s.resolve(Some(99)).map(|s| s.id), Some(1));
        assert!(s.is_open(None) && s.is_open(Some(2)) && s.is_closed(Some(3)));
    }

    #[test]
    fn lookup_by_name_ignores_case_and_spaces() {
        assert_eq!(set().by_name("  doing ").map(|s| s.id), Some(2));
        assert!(set().by_name("later").is_none());
    }

    #[test]
    fn only_rrggbb_is_a_colour() {
        assert!(is_hex("#a1B2c3"));
        for bad in ["", "#abc", "a1b2c3", "#a1b2c", "#gggggg", "#a1b2c3d"] {
            assert!(!is_hex(bad), "{bad}");
        }
    }

    #[test]
    fn an_empty_set_has_nothing_to_resolve() {
        assert!(Statuses::default().resolve(Some(1)).is_none());
        assert!(Statuses::default().is_open(None));
    }
}
