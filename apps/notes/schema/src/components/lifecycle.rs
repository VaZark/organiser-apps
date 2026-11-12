//! Creating, editing, pinning, archiving and trashing notes. Trashing is soft: a note stays
//! recoverable until it is purged.

use organiser_engine::{Db, Error, ItemPatch, NewItem, ParentFilter, Query, Result};
use serde_json::Value;

use crate::primitives::color::Color;
use crate::primitives::folder::{Folder, FolderEdit};
use crate::primitives::note::{Edit, Note};
use crate::primitives::section::{Section, SectionEdit};
use crate::{FOLDER, NOTE, SECTION};

#[derive(Debug, Clone, Default)]
pub struct NewNote {
    pub title: String,
    pub body: String,
    pub color: Option<Color>,
    pub tags: Vec<String>,
    /// The folder to file it in; none for a note that is in no folder.
    pub folder: Option<i64>,
    pub favorite: bool,
    /// Only where the folder offers ratings.
    pub rating: Option<u8>,
    /// One of the folder's sections; ignored without a folder.
    pub section: Option<i64>,
}

impl NewNote {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.into(),
            ..Self::default()
        }
    }
}

/// What the default folder is called.
pub const DEFAULT_FOLDER_TITLE: &str = "Unassigned";

/// The default folder, "Unassigned", where a note goes that is in no other: it is made when it is
/// missing, and every note in no folder at all is filed in it. It cannot be deleted. Returns its
/// id. Call it when the database is opened; it changes nothing when all is as it should be.
pub fn ensure_default_folder(db: &mut Db) -> Result<i64> {
    let existing = db
        .list(&Query {
            kind: Some(FOLDER.into()),
            ..Query::default()
        })?
        .iter()
        .map(Folder::from)
        .find(|f| f.is_default)
        .map(|f| f.id);
    let id = match existing {
        Some(id) => id,
        None => {
            db.create(
                NewItem::new(FOLDER, DEFAULT_FOLDER_TITLE)
                    .field("rated", false)
                    .field("default", true),
            )?
            .id
        }
    };
    // Notes made without a folder (by another tool) are in the default one.
    let loose = db.list(&Query {
        kind: Some(NOTE.into()),
        parent: ParentFilter::Root,
        ..Query::default()
    })?;
    for note in loose {
        db.move_item(note.id, Some(id), None)?;
    }
    Ok(id)
}

pub fn create(db: &mut Db, n: &NewNote) -> Result<Note> {
    let mut new = NewItem::new(NOTE, &n.title);
    // A note without a folder goes in the default one.
    let folder = match n.folder {
        Some(f) => f,
        None => ensure_default_folder(db)?,
    };
    new.parent_id = Some(folder);
    new.tags = n.tags.clone();
    if n.favorite {
        new = new.field("favorite", true);
    }
    if let Some(r) = n.rating {
        require_rated(db, Some(folder))?;
        new = new.field("rating", r);
    }
    if let Some(section) = n.section {
        check_section(db, section, folder)?;
        new = new.field("section", section);
    }
    if !n.body.is_empty() {
        new = new.field("body", n.body.as_str());
    }
    if let Some(c) = n.color {
        new = new.field("color", c.hex());
    }
    Ok(Note::from(&db.create(new)?))
}

/// An error unless `folder` exists and offers ratings: only there is a rating set.
fn require_rated(db: &Db, folder: Option<i64>) -> Result<()> {
    match folder {
        Some(f) if Folder::from(&db.get(f)?).rated => Ok(()),
        _ => Err(Error::Invalid(
            "that note's folder does not offer ratings".into(),
        )),
    }
}

/// The section must be one of that folder's.
fn check_section(db: &Db, section: i64, folder: i64) -> Result<()> {
    let item = db.get(section)?;
    if item.kind != SECTION || item.parent_id != Some(folder) {
        return Err(Error::Invalid(format!(
            "item {section} is not a section of folder {folder}"
        )));
    }
    Ok(())
}

/// Edit a note. A rating is only set where its folder offers them (clearing one always works). A
/// section must be one of the note's folder's.
pub fn edit(db: &mut Db, id: i64, edit: &Edit) -> Result<Note> {
    if let Some(Some(_)) = edit.rating {
        require_rated(db, db.get(id)?.parent_id)?;
    }
    if let Some(Some(section)) = edit.section {
        match db.get(id)?.parent_id {
            Some(folder) => check_section(db, section, folder)?,
            None => return Err(Error::Invalid("a section needs a folder".into())),
        }
    }
    Ok(Note::from(&db.update(id, edit.to_patch())?))
}

/// File a note in a folder, or (`None`) in the default one, "Unassigned". Its rating stays: it is shown again wherever the
/// note is in a folder that offers ratings. Its section does not: that is the old folder's.
pub fn move_note(db: &mut Db, id: i64, to: impl Into<Option<i64>>) -> Result<Note> {
    // Out of a folder is into the default one.
    let to = match to.into() {
        Some(f) => Some(f),
        None => Some(ensure_default_folder(db)?),
    };
    if Note::from(&db.get(id)?).folder_id != to {
        db.update(id, ItemPatch::default().field("section", Value::Null))?;
    }
    Ok(Note::from(&db.move_item(id, to, None)?))
}

/// Put a note in a section of its folder, or (`None`) in none.
pub fn set_section(db: &mut Db, id: i64, section: Option<i64>) -> Result<Note> {
    edit(
        db,
        id,
        &Edit {
            section: Some(section),
            ..Edit::default()
        },
    )
}

/// Mark a note as a favourite, or take the mark off.
pub fn set_favorite(db: &mut Db, id: i64, favorite: bool) -> Result<Note> {
    edit(
        db,
        id,
        &Edit {
            favorite: Some(favorite),
            ..Edit::default()
        },
    )
}

// ---- folders -------------------------------------------------------------------------------

pub fn create_folder(db: &mut Db, title: &str, rated: bool) -> Result<Folder> {
    let new = NewItem::new(FOLDER, title).field("rated", rated);
    Ok(Folder::from(&db.create(new)?))
}

/// Edit a folder. Switching `rated` off keeps the ratings its notes have: they are just not shown
/// or used until it is switched on again.
pub fn edit_folder(db: &mut Db, id: i64, edit: &FolderEdit) -> Result<Folder> {
    if db.get(id)?.kind != FOLDER {
        return Err(Error::Invalid(format!("item {id} is not a folder")));
    }
    Ok(Folder::from(&db.update(id, edit.to_patch())?))
}

/// A folder that exists and may be deleted: not the default one.
fn deletable(db: &Db, id: i64) -> Result<()> {
    let item = db.get(id)?;
    if item.kind != FOLDER {
        return Err(Error::Invalid(format!("item {id} is not a folder")));
    }
    if Folder::from(&item).is_default {
        return Err(Error::Invalid(
            "the Unassigned folder cannot be deleted".into(),
        ));
    }
    Ok(())
}

/// Delete a folder. Its notes stay, in the default folder, "Unassigned".
pub fn delete_folder(db: &mut Db, id: i64) -> Result<()> {
    deletable(db, id)?;
    let home = ensure_default_folder(db)?;
    // Its notes stay (out of the folder, and out of its sections); its sections go with it.
    for child in db.children(Some(id))? {
        if child.kind == NOTE {
            db.update(child.id, ItemPatch::default().field("section", Value::Null))?;
            db.move_item(child.id, Some(home), None)?;
        }
    }
    db.delete(id)?;
    Ok(())
}

/// Delete a folder and every note in it, trashed ones too. Returns how many notes went.
pub fn delete_folder_and_notes(db: &mut Db, id: i64) -> Result<usize> {
    deletable(db, id)?;
    Ok(db.delete(id)?.saturating_sub(1))
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

// ---- sections ------------------------------------------------------------------------------

/// Add a section, last, to a folder.
pub fn create_section(db: &mut Db, folder: i64, title: &str) -> Result<Section> {
    if db.get(folder)?.kind != FOLDER {
        return Err(Error::Invalid(format!("item {folder} is not a folder")));
    }
    let mut new = NewItem::new(SECTION, title);
    new.parent_id = Some(folder);
    Ok(Section::from(&db.create(new)?))
}

pub fn edit_section(db: &mut Db, id: i64, edit: &SectionEdit) -> Result<Section> {
    if db.get(id)?.kind != SECTION {
        return Err(Error::Invalid(format!("item {id} is not a section")));
    }
    Ok(Section::from(&db.update(id, edit.to_patch())?))
}

/// Move a section to place `to` (0 is first) among its folder's sections, the others keeping their
/// order. A place past the last is the last.
pub fn move_section(db: &mut Db, id: i64, to: usize) -> Result<()> {
    let item = db.get(id)?;
    if item.kind != SECTION {
        return Err(Error::Invalid(format!("item {id} is not a section")));
    }
    let parent = item.parent_id;
    let others: Vec<_> = db
        .children(parent)?
        .into_iter()
        .filter(|i| i.kind == SECTION && i.id != id)
        .collect();
    match others.get(to) {
        // Before the section now in that place.
        Some(next) => db.move_item(id, parent, Some(next.position))?,
        None => db.move_item(id, parent, None)?,
    };
    Ok(())
}

/// Delete a section. Its notes stay in the folder, in no section.
pub fn delete_section(db: &mut Db, id: i64) -> Result<()> {
    if db.get(id)?.kind != SECTION {
        return Err(Error::Invalid(format!("item {id} is not a section")));
    }
    db.delete(id)?;
    Ok(())
}

/// Move a folder before `before` (another folder), or last when there is none: the others keep
/// their order. (The default folder is always listed last whatever its place.)
pub fn move_folder(db: &mut Db, id: i64, before: Option<i64>) -> Result<()> {
    if db.get(id)?.kind != FOLDER {
        return Err(Error::Invalid(format!("item {id} is not a folder")));
    }
    match before.filter(|b| *b != id) {
        Some(b) => {
            let next = db.get(b)?;
            if next.kind != FOLDER {
                return Err(Error::Invalid(format!("item {b} is not a folder")));
            }
            db.move_item(id, None, Some(next.position))?;
        }
        None if before.is_some() => {}
        None => {
            db.move_item(id, None, None)?;
        }
    }
    Ok(())
}
