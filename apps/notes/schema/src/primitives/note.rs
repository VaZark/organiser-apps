//! The typed view of a note item, and the patch for editing one.

use organiser_engine::{Item, ItemPatch};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::color::Color;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub color: Option<Color>,
    pub pinned: bool,
    /// Shown as a desktop sticky by the Sticky app.
    #[serde(default)]
    pub sticky: bool,
    pub archived: bool,
    /// Unix seconds when trashed.
    pub trashed: Option<i64>,
    pub tags: Vec<String>,
    pub created: i64,
    pub updated: i64,
}

impl Note {
    pub fn is_trashed(&self) -> bool {
        self.trashed.is_some()
    }
}

impl From<&Item> for Note {
    fn from(i: &Item) -> Self {
        let f = &i.fields;
        Self {
            id: i.id,
            title: i.title.clone(),
            body: f.get("body").and_then(Value::as_str).unwrap_or("").into(),
            color: f
                .get("color")
                .and_then(Value::as_str)
                .and_then(Color::parse),
            pinned: f.get("pinned").and_then(Value::as_bool).unwrap_or(false),
            sticky: f.get("sticky").and_then(Value::as_bool).unwrap_or(false),
            archived: i.archived,
            trashed: f.get("trashed").and_then(Value::as_i64),
            tags: i.tags.clone(),
            created: i.created,
            updated: i.updated,
        }
    }
}

/// Fields to change. `None` leaves a field alone; `Some(None)` clears it.
#[derive(Debug, Clone, Default)]
pub struct Edit {
    pub title: Option<String>,
    pub body: Option<String>,
    pub color: Option<Option<Color>>,
}

impl Edit {
    pub fn to_patch(&self) -> ItemPatch {
        let mut p = ItemPatch::default();
        if let Some(t) = &self.title {
            p = p.title(t);
        }
        if let Some(b) = &self.body {
            // An empty body is stored as no body.
            p = p.field(
                "body",
                if b.is_empty() {
                    Value::Null
                } else {
                    b.clone().into()
                },
            );
        }
        if let Some(c) = self.color {
            p = p.field("color", c.map_or(Value::Null, |c| c.hex().into()));
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_body_and_cleared_color_become_nulls() {
        let p = Edit {
            body: Some(String::new()),
            color: Some(None),
            ..Edit::default()
        }
        .to_patch();
        assert_eq!(p.fields["body"], Value::Null);
        assert_eq!(p.fields["color"], Value::Null);
        assert!(p.title.is_none());
    }

    #[test]
    fn color_is_stored_as_hex() {
        let p = Edit {
            color: Some(Some(Color::rgb(1, 2, 3))),
            ..Edit::default()
        }
        .to_patch();
        assert_eq!(p.fields["color"], "#010203");
    }
}
