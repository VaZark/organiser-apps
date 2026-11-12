//! Creating, editing, pinning, archiving and trashing notes. Trashing is soft: a note stays
//! recoverable until it is purged.

use organiser_engine::{Db, Error, ItemPatch, NewItem, Result};
use serde_json::Value;

use crate::NOTE;
use crate::primitives::color::Color;
use crate::primitives::note::{Edit, Note};

#[derive(Debug, Clone, Default)]
pub struct NewNote {
    pub title: String,
    pub body: String,
    pub color: Option<Color>,
    pub tags: Vec<String>,
}

impl NewNote {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.into(),
            ..Self::default()
        }
    }
}

pub fn create(db: &mut Db, n: &NewNote) -> Result<Note> {
    let mut new = NewItem::new(NOTE, &n.title);
    new.tags = n.tags.clone();
    if !n.body.is_empty() {
        new = new.field("body", n.body.as_str());
    }
    if let Some(c) = n.color {
        new = new.field("color", c.hex());
    }
    Ok(Note::from(&db.create(new)?))
}

pub fn edit(db: &mut Db, id: i64, edit: &Edit) -> Result<Note> {
    Ok(Note::from(&db.update(id, edit.to_patch())?))
}

/// Make the note's tags exactly `tags`.
pub fn set_tags(db: &mut Db, id: i64, tags: &[String]) -> Result<()> {
    db.set_tags(id, tags).map(|_| ())
}

pub fn set_pinned(db: &mut Db, id: i64, pinned: bool) -> Result<Note> {
    let v = if pinned {
        Value::Bool(true)
    } else {
        Value::Null
    };
    Ok(Note::from(
        &db.update(id, ItemPatch::default().field("pinned", v))?,
    ))
}

pub fn set_archived(db: &mut Db, id: i64, archived: bool) -> Result<Note> {
    Ok(Note::from(
        &db.update(id, ItemPatch::default().archived(archived))?,
    ))
}

/// Move a note to the trash at `now` (unix seconds). Pinning is dropped: a trashed note is not
/// something to keep at hand.
pub fn trash(db: &mut Db, id: i64, now: i64) -> Result<Note> {
    let patch = ItemPatch::default()
        .field("trashed", now)
        .field("pinned", Value::Null)
        .field("sticky", Value::Null)
        .field("sticky_color", Value::Null);
    Ok(Note::from(&db.update(id, patch)?))
}

pub fn restore(db: &mut Db, id: i64) -> Result<Note> {
    let patch = ItemPatch::default().field("trashed", Value::Null);
    Ok(Note::from(&db.update(id, patch)?))
}

/// Delete one note for good. Only a trashed note can be: it must go through the trash first.
pub fn purge(db: &mut Db, id: i64) -> Result<()> {
    if !Note::from(&db.get(id)?).is_trashed() {
        return Err(Error::Invalid(format!(
            "note {id} is not in the trash; trash it first"
        )));
    }
    db.delete(id)?;
    Ok(())
}

/// Delete every note trashed at or before `before` (unix seconds). Returns how many.
pub fn purge_older_than(db: &mut Db, before: i64) -> Result<usize> {
    let old: Vec<i64> = crate::views::trashed(db)?
        .into_iter()
        .filter(|n| n.trashed.is_some_and(|t| t <= before))
        .map(|n| n.id)
        .collect();
    for id in &old {
        db.delete(*id)?;
    }
    Ok(old.len())
}

/// Delete everything in the trash. Returns how many.
pub fn empty_trash(db: &mut Db) -> Result<usize> {
    purge_older_than(db, i64::MAX)
}

/// How long a note stays in the trash unless the settings say otherwise.
pub const TRASH_DAYS: i64 = 30;

/// Delete notes that have been in the trash for `days` or more. Returns how many.
pub fn purge_expired(db: &mut Db, now: i64, days: u32) -> Result<usize> {
    purge_older_than(db, now - i64::from(days) * 86_400)
}
