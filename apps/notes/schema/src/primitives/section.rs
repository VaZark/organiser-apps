//! Sections: named, coloured groups of notes inside a folder, each with a description. A note
//! belongs to at most one section of its folder.

use organiser_engine::{Item, ItemPatch};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::color::Color;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub id: i64,
    /// The folder it is in.
    pub folder: i64,
    pub title: String,
    /// What the section is for, Markdown.
    pub notes: String,
    pub color: Option<Color>,
    pub position: i64,
}

impl From<&Item> for Section {
    fn from(i: &Item) -> Self {
        let f = &i.fields;
        Self {
            id: i.id,
            folder: i.parent_id.unwrap_or_default(),
            title: i.title.clone(),
            notes: f.get("notes").and_then(Value::as_str).unwrap_or("").into(),
            color: f.get("color").and_then(Value::as_str).and_then(Color::parse),
            position: i.position,
        }
    }
}

/// A section with how many live notes it holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionSummary {
    pub section: Section,
    pub notes: usize,
}

/// A change to a section. `None` leaves a field alone; `Some(None)` clears a colour.
#[derive(Debug, Clone, Default)]
pub struct SectionEdit {
    pub title: Option<String>,
    pub notes: Option<String>,
    pub color: Option<Option<Color>>,
}

impl SectionEdit {
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
        if let Some(c) = self.color {
            p.fields
                .insert("color".into(), c.map_or(Value::Null, |c| c.hex().into()));
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cleared_colour_and_empty_description_become_nulls() {
        let p = SectionEdit {
            notes: Some(String::new()),
            color: Some(None),
            ..SectionEdit::default()
        }
        .to_patch();
        assert_eq!(p.fields["notes"], Value::Null);
        assert_eq!(p.fields["color"], Value::Null);
    }
}
