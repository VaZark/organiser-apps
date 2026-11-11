//! Creating and editing lists and entries.

use organiser_engine::{Db, Error, ItemPatch, NewItem, Result};
use serde_json::Value;

use crate::primitives::model::{Entry, EntryEdit, List, ListEdit};
use crate::{ENTRY, LIST};

pub fn create_list(db: &mut Db, title: &str, rated: bool) -> Result<List> {
    let new = NewItem::new(LIST, title).field("rated", rated);
    Ok(List::from(&db.create(new)?))
}

/// Edit a list. Switching `rated` off removes the ratings its entries had; the engine does that,
/// so it holds for every frontend.
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

/// Add an entry to a list. A rating is refused if the list does not offer them.
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

/// Move an entry to another list. A rating the new list does not offer is dropped first.
pub fn move_entry(db: &mut Db, id: i64, to: i64) -> Result<Entry> {
    let dest = List::from(&db.get(to)?);
    let mut patch = ItemPatch::default();
    if !dest.rated {
        patch.fields.insert("rating".into(), Value::Null);
    }
    db.update(id, patch)?;
    Ok(Entry::from(&db.move_item(id, Some(to), None)?))
}

pub fn set_tags(db: &mut Db, id: i64, tags: &[String]) -> Result<()> {
    db.set_tags(id, tags).map(|_| ())
}

/// Clear what older versions kept and this one does not: lists had a `tracked` switch and entries
/// a `status`. The engine refuses fields its schema no longer knows, so an item that still has
/// one could not be edited. Call it once after opening; it writes only where there is something
/// to clear, and returns how many items it touched.
pub fn drop_legacy(db: &mut Db) -> Result<usize> {
    let mut touched = 0;
    for (kind, field) in [(LIST, "tracked"), (ENTRY, "status")] {
        let old: Vec<i64> = db
            .list(&organiser_engine::Query {
                kind: Some(kind.into()),
                ..organiser_engine::Query::default()
            })?
            .into_iter()
            .filter(|i| i.fields.contains_key(field))
            .map(|i| i.id)
            .collect();
        for id in old {
            db.update(id, ItemPatch::default().field(field, Value::Null))?;
            touched += 1;
        }
    }
    Ok(touched)
}
