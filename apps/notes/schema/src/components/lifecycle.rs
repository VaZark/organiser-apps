//! Creating, editing, pinning, archiving and trashing notes. Trashing is soft: a note stays
//! recoverable until it is purged.

use organiser_engine::{Db, Error, ItemPatch, NewItem, ParentFilter, Query, Result};
use serde_json::Value;

use crate::primitives::color::Color;
use crate::primitives::folder::{Folder, FolderEdit};
use crate::primitives::note::{Edit, Note};
use crate::{FOLDER, NOTE, SECTION};
use organiser_engine::{FieldFilter, Op};

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
    migrate_sections(db)?;
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

/// Edit a note. A rating is only set where its folder offers them (clearing one always works).
pub fn edit(db: &mut Db, id: i64, edit: &Edit) -> Result<Note> {
    if let Some(Some(_)) = edit.rating {
        require_rated(db, db.get(id)?.parent_id)?;
    }
    Ok(Note::from(&db.update(id, edit.to_patch())?))
}

/// File a note in a folder, or (`None`) in the default one, "Unassigned". Its rating stays: it is
/// shown again wherever the note is in a folder that offers ratings.
pub fn move_note(db: &mut Db, id: i64, to: impl Into<Option<i64>>) -> Result<Note> {
    // Out of a folder is into the default one.
    let to = match to.into() {
        Some(f) => Some(f),
        None => Some(ensure_default_folder(db)?),
    };
    if let Some(f) = to
        && db.get(f)?.kind != FOLDER
    {
        return Err(Error::Invalid(format!("item {f} is not a folder")));
    }
    Ok(Note::from(&db.move_item(id, to, None)?))
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

/// A folder at the top.
pub fn create_folder(db: &mut Db, title: &str, rated: bool) -> Result<Folder> {
    create_folder_in(db, None, title, rated)
}

/// A folder, last, in `parent` (or at the top). Folders nest to any depth, but not in "Unassigned".
pub fn create_folder_in(db: &mut Db, parent: Option<i64>, title: &str, rated: bool) -> Result<Folder> {
    if let Some(p) = parent {
        let item = db.get(p)?;
        if item.kind != FOLDER {
            return Err(Error::Invalid(format!("item {p} is not a folder")));
        }
        if Folder::from(&item).is_default {
            return Err(Error::Invalid("the Unassigned folder cannot hold folders".into()));
        }
    }
    let mut new = NewItem::new(FOLDER, title).field("rated", rated);
    new.parent_id = parent;
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

/// Every folder below `id`, at any depth, nearest first.
pub fn descendants(db: &Db, id: i64) -> Result<Vec<Folder>> {
    let mut out = Vec::new();
    let mut next = vec![id];
    while let Some(at) = next.pop() {
        for c in db.children(Some(at))? {
            if c.kind == FOLDER {
                next.push(c.id);
                out.push(Folder::from(&c));
            }
        }
    }
    Ok(out)
}

/// The folders from the top down to `id`, itself last.
pub fn folder_path(db: &Db, id: i64) -> Result<Vec<Folder>> {
    let mut path = Vec::new();
    let mut at = Some(id);
    while let Some(i) = at {
        let item = db.get(i)?;
        if item.kind != FOLDER || path.len() > 256 {
            break;
        }
        at = item.parent_id;
        path.push(Folder::from(&item));
    }
    path.reverse();
    Ok(path)
}

/// Delete a folder. Its notes and its folders stay, in the folder it was in (the default folder,
/// "Unassigned", for a notes of one at the top).
pub fn delete_folder(db: &mut Db, id: i64) -> Result<()> {
    deletable(db, id)?;
    let parent = db.get(id)?.parent_id;
    let home = match parent {
        Some(p) => p,
        None => ensure_default_folder(db)?,
    };
    for child in db.children(Some(id))? {
        match child.kind.as_str() {
            NOTE => {
                db.move_item(child.id, Some(home), None)?;
            }
            FOLDER => {
                db.move_item(child.id, parent, None)?;
            }
            _ => {}
        }
    }
    db.delete(id)?;
    Ok(())
}

/// Delete a folder, the folders in it and every note in them, trashed ones too. Returns how many
/// notes went.
pub fn delete_folder_and_notes(db: &mut Db, id: i64) -> Result<usize> {
    deletable(db, id)?;
    let folders = 1 + descendants(db, id)?.len();
    Ok(db.delete(id)?.saturating_sub(folders))
}

/// Put a folder inside `parent` (or at the top), last. Not inside itself or what is in it, and
/// not inside "Unassigned"; "Unassigned" itself stays at the top.
pub fn move_folder_into(db: &mut Db, id: i64, parent: Option<i64>) -> Result<()> {
    let item = db.get(id)?;
    if item.kind != FOLDER {
        return Err(Error::Invalid(format!("item {id} is not a folder")));
    }
    if Folder::from(&item).is_default {
        return Err(Error::Invalid("the Unassigned folder stays at the top".into()));
    }
    if let Some(p) = parent {
        let target = db.get(p)?;
        if target.kind != FOLDER {
            return Err(Error::Invalid(format!("item {p} is not a folder")));
        }
        if Folder::from(&target).is_default {
            return Err(Error::Invalid("the Unassigned folder cannot hold folders".into()));
        }
        if p == id || descendants(db, id)?.iter().any(|f| f.id == p) {
            return Err(Error::Invalid(
                "a folder cannot go inside itself or a folder in it".into(),
            ));
        }
    }
    db.move_item(id, parent, None)?;
    Ok(())
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

// ---- what sections were ---------------------------------------------------------------------

/// Turn every section into a folder inside its folder, with its title, description and colour, in
/// its place among the others, and move its notes into it (they keep their rating where the
/// folder rates). Changes nothing when there are no sections, so it is safe to run at every start.
/// Returns how many there were.
pub fn migrate_sections(db: &mut Db) -> Result<usize> {
    let sections = db.list(&Query {
        kind: Some(SECTION.into()),
        sort: vec![organiser_engine::Sort::asc("position")],
        ..Query::default()
    })?;
    for s in &sections {
        let rated = match s.parent_id {
            Some(p) => Folder::from(&db.get(p)?).rated,
            None => false,
        };
        let mut new = NewItem::new(FOLDER, &s.title).field("rated", rated);
        new.parent_id = s.parent_id;
        if let Some(n) = s.fields.get("notes").and_then(Value::as_str).filter(|n| !n.is_empty()) {
            new = new.field("notes", n);
        }
        if let Some(c) = s.fields.get("color").and_then(Value::as_str) {
            new = new.field("color", c);
        }
        let folder = db.create(new)?;
        let members = db.list(&Query {
            kind: Some(NOTE.into()),
            filters: vec![FieldFilter::new("section", Op::Eq, s.id)],
            ..Query::default()
        })?;
        for n in members {
            db.update(n.id, ItemPatch::default().field("section", Value::Null))?;
            db.move_item(n.id, Some(folder.id), None)?;
        }
        db.delete(s.id)?;
    }
    Ok(sections.len())
}

/// Move a folder before `before` (another folder in the same folder), or last there when there is
/// none: the others keep their order. (The default folder is always listed last whatever its place.)
pub fn move_folder(db: &mut Db, id: i64, before: Option<i64>) -> Result<()> {
    let item = db.get(id)?;
    if item.kind != FOLDER {
        return Err(Error::Invalid(format!("item {id} is not a folder")));
    }
    let parent = item.parent_id;
    match before.filter(|b| *b != id) {
        Some(b) => {
            let next = db.get(b)?;
            if next.kind != FOLDER {
                return Err(Error::Invalid(format!("item {b} is not a folder")));
            }
            if next.parent_id != parent {
                return Err(Error::Invalid(
                    "a folder is moved among the folders it is with".into(),
                ));
            }
            db.move_item(id, parent, Some(next.position))?;
        }
        None if before.is_some() => {}
        None => {
            db.move_item(id, parent, None)?;
        }
    }
    Ok(())
}
