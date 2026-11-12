//! Folders: named, coloured places notes can be filed in, each choosing whether its notes can be
//! rated. A note needs no folder.

use organiser_engine::{Item, ItemPatch};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::color::Color;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Folder {
    pub id: i64,
    pub title: String,
    pub notes: String,
    /// Its notes can be rated.
    pub rated: bool,
    pub color: Option<Color>,
    /// The name of its icon; none is the folder.
    pub icon: Option<String>,
    /// The folder of the notes that are in no other: it cannot be deleted.
    pub is_default: bool,
    pub position: i64,
    pub created: i64,
    pub updated: i64,
}

impl From<&Item> for Folder {
    fn from(i: &Item) -> Self {
        let f = &i.fields;
        Self {
            id: i.id,
            title: i.title.clone(),
            notes: f.get("notes").and_then(Value::as_str).unwrap_or("").into(),
            rated: f.get("rated").and_then(Value::as_bool).unwrap_or(false),
            color: f.get("color").and_then(Value::as_str).and_then(Color::parse),
            icon: f.get("icon").and_then(Value::as_str).map(String::from),
            is_default: f.get("default").and_then(Value::as_bool).unwrap_or(false),
            position: i.position,
            created: i.created,
            updated: i.updated,
        }
    }
}

/// A folder with how many live notes it holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FolderSummary {
    pub folder: Folder,
    pub notes: usize,
    pub favorites: usize,
}

/// A change to a folder. `None` leaves a field alone; `Some(None)` clears a colour.
#[derive(Debug, Clone, Default)]
pub struct FolderEdit {
    pub title: Option<String>,
    pub notes: Option<String>,
    pub rated: Option<bool>,
    pub color: Option<Option<Color>>,
    /// The icon's name; `Some(None)` clears it.
    pub icon: Option<Option<String>>,
}

impl FolderEdit {
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
        if let Some(v) = self.rated {
            p.fields.insert("rated".into(), v.into());
        }
        if let Some(c) = self.color {
            p.fields
                .insert("color".into(), c.map_or(Value::Null, |c| c.hex().into()));
        }
        if let Some(i) = &self.icon {
            p.fields
                .insert("icon".into(), i.clone().map_or(Value::Null, Value::from));
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cleared_colour_and_empty_notes_become_nulls() {
        let p = FolderEdit {
            notes: Some(String::new()),
            color: Some(None),
            rated: Some(true),
            ..FolderEdit::default()
        }
        .to_patch();
        assert_eq!(p.fields["notes"], Value::Null);
        assert_eq!(p.fields["color"], Value::Null);
        assert_eq!(p.fields["rated"], Value::Bool(true));
    }
}
