//! Creating and editing lists and entries.

use organiser_engine::{Db, Error, ItemPatch, NewItem, Result};
use serde_json::Value;

use crate::primitives::model::{Entry, EntryEdit, List, ListEdit};
use crate::primitives::status::Status;
use crate::{ENTRY, LIST};

pub fn create_list(db: &mut Db, title: &str, rated: bool, tracked: bool) -> Result<List> {
    let new = NewItem::new(LIST, title)
        .field("rated", rated)
        .field("tracked", tracked);
    Ok(List::from(&db.create(new)?))
}

/// Edit a list. Switching `rated` or `tracked` off removes the ratings or statuses its entries
/// had; the engine does that, so it holds for every frontend.
pub fn edit_list(db: &mut Db, id: i64, edit: &ListEdit) -> Result<List> {
    Ok(List::from(&db.update(id, edit.to_patch())?))
}

/// Delete a list and every entry in it. Returns how many items went.
pub fn delete_list(db: &mut Db, id: i64) -> Result<usize> {
    if db.get(id)?.kind != LIST {
        return Err(Error::Invalid(format!("item {id} is not a list")));
    }
    db.delete(id)
}

#[derive(Debug, Clone, Default)]
pub struct NewEntry {
    pub title: String,
    pub notes: Option<String>,
    pub favorite: bool,
    pub rating: Option<u8>,
    pub status: Option<Status>,
    pub tags: Vec<String>,
}

impl NewEntry {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.into(),
            ..Self::default()
        }
    }
}

/// Add an entry to a list. A rating or status is refused if the list does not offer them.
pub fn add_entry(db: &mut Db, list: i64, e: &NewEntry) -> Result<Entry> {
    let mut new = NewItem::new(ENTRY, &e.title).parent(list);
    new.tags = e.tags.clone();
    if let Some(n) = &e.notes {
        new = new.field("notes", n.clone());
    }
    if e.favorite {
        new = new.field("favorite", true);
    }
    if let Some(r) = e.rating {
        new = new.field("rating", r);
    }
    if let Some(s) = e.status {
        new = new.field("status", s.as_str());
    }
    Ok(Entry::from(&db.create(new)?))
}

pub fn edit_entry(db: &mut Db, id: i64, edit: &EntryEdit) -> Result<Entry> {
    Ok(Entry::from(&db.update(id, edit.to_patch())?))
}

pub fn set_favorite(db: &mut Db, id: i64, favorite: bool) -> Result<Entry> {
    edit_entry(
        db,
        id,
        &EntryEdit {
            favorite: Some(favorite),
            ..EntryEdit::default()
        },
    )
}

/// Move an entry to another list. What the new list does not offer (a rating, a status) is
/// dropped from the entry first.
pub fn move_entry(db: &mut Db, id: i64, to: i64) -> Result<Entry> {
    let dest = List::from(&db.get(to)?);
    let mut patch = ItemPatch::default();
    if !dest.rated {
        patch.fields.insert("rating".into(), Value::Null);
    }
    if !dest.tracked {
        patch.fields.insert("status".into(), Value::Null);
    }
    db.update(id, patch)?;
    Ok(Entry::from(&db.move_item(id, Some(to), None)?))
}

pub fn set_tags(db: &mut Db, id: i64, tags: &[String]) -> Result<()> {
    db.set_tags(id, tags).map(|_| ())
}
