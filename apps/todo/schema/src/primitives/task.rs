//! The typed views of task and epic items, and the patch that edits them.

use chrono::NaiveDate;
use organiser_engine::{Item, ItemPatch};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::priority::Priority;

fn date(item: &Item, key: &str) -> Option<NaiveDate> {
    item.fields.get(key)?.as_str()?.parse().ok()
}

fn text(item: &Item, key: &str) -> String {
    item.fields
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// The id of the status an item points at, if it has one.
fn status(item: &Item) -> Option<i64> {
    item.fields.get("status").and_then(Value::as_i64)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub parent_id: Option<i64>,
    /// The epic this task belongs to.
    pub epic: Option<i64>,
    pub title: String,
    pub notes: String,
    /// The status this is in (see [`Statuses`](super::status::Statuses)); `None` counts as the
    /// first open one.
    pub status: Option<i64>,
    pub priority: Priority,
    pub start: Option<NaiveDate>,
    pub due: Option<NaiveDate>,
    pub tags: Vec<String>,
    pub position: i64,
    pub archived: bool,
    pub created: i64,
    pub updated: i64,
}

impl From<&Item> for Task {
    fn from(i: &Item) -> Self {
        Self {
            id: i.id,
            parent_id: i.parent_id,
            epic: i.fields.get("epic").and_then(Value::as_i64),
            title: i.title.clone(),
            notes: text(i, "notes"),
            status: status(i),
            priority: i
                .fields
                .get("priority")
                .and_then(Value::as_str)
                .and_then(Priority::parse)
                .unwrap_or_default(),
            start: date(i, "start"),
            due: date(i, "due"),
            tags: i.tags.clone(),
            position: i.position,
            archived: i.archived,
            created: i.created,
            updated: i.updated,
        }
    }
}

impl Task {
    /// Whether the task is happening today: it has a start or a due date, and today falls within
    /// them. A missing start counts as already started and a missing due date as never ending.
    /// A task with no dates at all is not scheduled, so it is not current, and an overdue task is
    /// late rather than current.
    pub fn is_current(&self, today: NaiveDate) -> bool {
        (self.start.is_some() || self.due.is_some())
            && self.start.is_none_or(|s| s <= today)
            && self.due.is_none_or(|d| d >= today)
    }
}

/// A group of tasks working towards one goal, with its own dates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Epic {
    pub id: i64,
    pub title: String,
    pub notes: String,
    /// The status this is in (see [`Statuses`](super::status::Statuses)); `None` counts as the
    /// first open one.
    pub status: Option<i64>,
    pub start: Option<NaiveDate>,
    pub due: Option<NaiveDate>,
    pub tags: Vec<String>,
    pub created: i64,
    pub updated: i64,
}

impl From<&Item> for Epic {
    fn from(i: &Item) -> Self {
        Self {
            id: i.id,
            title: i.title.clone(),
            notes: text(i, "notes"),
            status: status(i),
            start: date(i, "start"),
            due: date(i, "due"),
            tags: i.tags.clone(),
            created: i.created,
            updated: i.updated,
        }
    }
}

/// An epic with how far along its tasks are.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpicSummary {
    pub epic: Epic,
    /// Tasks not yet done or dismissed.
    pub open: usize,
    pub total: usize,
}

impl EpicSummary {
    /// 0.0..=1.0; an epic with no tasks has made no progress.
    pub fn progress(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.total - self.open) as f32 / self.total as f32
        }
    }
}

/// What to change. `None` leaves a field alone; `Some(None)` clears a date or the epic.
#[derive(Debug, Clone, Default)]
pub struct Edit {
    pub title: Option<String>,
    pub notes: Option<String>,
    pub start: Option<Option<NaiveDate>>,
    pub due: Option<Option<NaiveDate>>,
    pub priority: Option<Priority>,
    pub epic: Option<Option<i64>>,
    pub archived: Option<bool>,
}

impl Edit {
    pub fn to_patch(&self) -> ItemPatch {
        let mut p = ItemPatch {
            title: self.title.clone(),
            archived: self.archived,
            ..ItemPatch::default()
        };
        if let Some(n) = &self.notes {
            p.fields.insert(
                "notes".into(),
                if n.is_empty() {
                    Value::Null
                } else {
                    n.clone().into()
                },
            );
        }
        for (key, v) in [("start", self.start), ("due", self.due)] {
            if let Some(v) = v {
                p.fields
                    .insert(key.into(), v.map_or(Value::Null, |d| d.to_string().into()));
            }
        }
        if let Some(pr) = self.priority {
            p.fields.insert("priority".into(), pr.as_str().into());
        }
        if let Some(e) = self.epic {
            p.fields
                .insert("epic".into(), e.map_or(Value::Null, Value::from));
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_is_the_closed_share() {
        let epic = Epic {
            id: 1,
            title: "e".into(),
            notes: String::new(),
            status: None,
            start: None,
            due: None,
            tags: vec![],
            created: 0,
            updated: 0,
        };
        let s = |open, total| EpicSummary {
            epic: epic.clone(),
            open,
            total,
        };
        assert_eq!(s(0, 0).progress(), 0.0);
        assert_eq!(s(1, 4).progress(), 0.75);
        assert_eq!(s(0, 3).progress(), 1.0);
    }
}
