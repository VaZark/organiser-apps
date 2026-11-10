//! Lists and entries as typed values, and the patches that edit them.

use organiser_engine::{Item, ItemPatch};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::status::Status;

fn text(i: &Item, key: &str) -> String {
    i.fields
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn flag(i: &Item, key: &str) -> bool {
    i.fields.get(key).and_then(Value::as_bool).unwrap_or(false)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct List {
    pub id: i64,
    pub title: String,
    pub notes: String,
    /// Its entries can be rated.
    pub rated: bool,
    /// Its entries have a status.
    pub tracked: bool,
    pub position: i64,
    pub created: i64,
    pub updated: i64,
}

impl From<&Item> for List {
    fn from(i: &Item) -> Self {
        Self {
            id: i.id,
            title: i.title.clone(),
            notes: text(i, "notes"),
            rated: flag(i, "rated"),
            tracked: flag(i, "tracked"),
            position: i.position,
            created: i.created,
            updated: i.updated,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub id: i64,
    pub list_id: i64,
    pub title: String,
    /// Notes, or the person's opinion.
    pub notes: String,
    pub favorite: bool,
    /// 1 to 5 stars; only in a list that is rated.
    pub rating: Option<u8>,
    /// Only in a list that is tracked.
    pub status: Option<Status>,
    pub tags: Vec<String>,
    pub position: i64,
    pub created: i64,
    pub updated: i64,
}

impl From<&Item> for Entry {
    fn from(i: &Item) -> Self {
        Self {
            id: i.id,
            list_id: i.parent_id.unwrap_or_default(),
            title: i.title.clone(),
            notes: text(i, "notes"),
            favorite: flag(i, "favorite"),
            rating: i
                .fields
                .get("rating")
                .and_then(Value::as_u64)
                .map(|r| r as u8),
            status: i
                .fields
                .get("status")
                .and_then(Value::as_str)
                .and_then(Status::parse),
            tags: i.tags.clone(),
            position: i.position,
            created: i.created,
            updated: i.updated,
        }
    }
}

/// A list with how many entries it has.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListSummary {
    pub list: List,
    pub entries: usize,
    pub favorites: usize,
    /// Entries marked done, in a tracked list.
    pub done: usize,
}

/// A change to a list. `None` leaves a field alone.
#[derive(Debug, Clone, Default)]
pub struct ListEdit {
    pub title: Option<String>,
    pub notes: Option<String>,
    pub rated: Option<bool>,
    pub tracked: Option<bool>,
}

impl ListEdit {
    pub fn to_patch(&self) -> ItemPatch {
        let mut p = ItemPatch {
            title: self.title.clone(),
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
        for (k, v) in [("rated", self.rated), ("tracked", self.tracked)] {
            if let Some(v) = v {
                p.fields.insert(k.into(), v.into());
            }
        }
        p
    }
}

/// A change to an entry. `None` leaves a field alone; `Some(None)` clears a rating or status.
#[derive(Debug, Clone, Default)]
pub struct EntryEdit {
    pub title: Option<String>,
    pub notes: Option<String>,
    pub favorite: Option<bool>,
    pub rating: Option<Option<u8>>,
    pub status: Option<Option<Status>>,
}

impl EntryEdit {
    pub fn to_patch(&self) -> ItemPatch {
        let mut p = ItemPatch {
            title: self.title.clone(),
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
        if let Some(f) = self.favorite {
            p.fields.insert("favorite".into(), f.into());
        }
        if let Some(r) = self.rating {
            p.fields
                .insert("rating".into(), r.map_or(Value::Null, Value::from));
        }
        if let Some(s) = self.status {
            p.fields.insert(
                "status".into(),
                s.map_or(Value::Null, |s| s.as_str().into()),
            );
        }
        p
    }
}
