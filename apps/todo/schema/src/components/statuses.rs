//! The person's statuses: loading them, and the rules for changing them. There is always at
//! least one open and one closed status, so a task can always be started and finished.

use organiser_engine::{Db, Error, ItemPatch, NewItem, Query, Result, Sort};
use serde_json::Value;

use crate::primitives::status::{DEFAULTS, Status, Statuses, is_hex};
use crate::{EPIC, STATUS, TASK};

pub fn load(db: &Db) -> Result<Statuses> {
    let q = Query {
        kind: Some(STATUS.into()),
        sort: vec![Sort::asc("position")],
        ..Query::default()
    };
    Ok(Statuses::new(
        db.list(&q)?.iter().map(Status::from).collect(),
    ))
}

/// Make sure the database has statuses, and that nothing still holds one by its old fixed name.
/// Call it once after opening: a new database gets To do, In progress, Done and Won't do, and a
/// database from before statuses were the person's own has its tasks and epics moved over.
pub fn ensure_defaults(db: &mut Db) -> Result<()> {
    if load(db)?.all().is_empty() {
        for (name, color, closed, _) in DEFAULTS {
            create(db, name, Some(color), closed)?;
        }
    }
    let statuses = load(db)?;
    for kind in [TASK, EPIC] {
        let items = db.list(&Query {
            kind: Some(kind.into()),
            ..Query::default()
        })?;
        for item in items {
            let Some(old) = item.fields.get("status").and_then(Value::as_str) else {
                continue;
            };
            let target = DEFAULTS
                .iter()
                .find(|d| d.3 == old)
                .and_then(|d| statuses.by_name(d.0))
                // Named differently now, or unknown: keep what it meant, open or finished.
                .or_else(|| match old {
                    "done" | "dismissed" => statuses.first_closed(),
                    _ => statuses.first_open(),
                });
            if let Some(t) = target {
                db.update(item.id, ItemPatch::default().field("status", t.id))?;
            }
        }
    }
    Ok(())
}

fn check_color(c: &str) -> Result<()> {
    if is_hex(c) {
        Ok(())
    } else {
        Err(Error::Invalid(format!(
            "`{c}` is not a colour; expected #rrggbb"
        )))
    }
}

pub fn create(db: &mut Db, name: &str, color: Option<&str>, closed: bool) -> Result<Status> {
    let mut new = NewItem::new(STATUS, name).field("closed", closed);
    if let Some(c) = color {
        check_color(c)?;
        new = new.field("color", c.to_ascii_lowercase());
    }
    if load(db)?.by_name(name).is_some() {
        return Err(Error::Invalid(format!("there is already a status {name}")));
    }
    Ok(Status::from(&db.create(new)?))
}

/// What to change. `None` leaves a field alone; `Some(None)` clears the colour.
#[derive(Debug, Clone, Default)]
pub struct StatusEdit {
    pub name: Option<String>,
    pub color: Option<Option<String>>,
    pub closed: Option<bool>,
}

pub fn edit(db: &mut Db, id: i64, e: &StatusEdit) -> Result<Status> {
    let all = load(db)?;
    let Some(current) = all.get(id) else {
        return Err(Error::NotFound(id));
    };
    let mut patch = ItemPatch::default();
    if let Some(name) = &e.name {
        if all
            .all()
            .iter()
            .any(|s| s.id != id && s.name.eq_ignore_ascii_case(name.trim()))
        {
            return Err(Error::Invalid(format!("there is already a status {name}")));
        }
        patch = patch.title(name);
    }
    if let Some(c) = &e.color {
        if let Some(c) = c {
            check_color(c)?;
        }
        patch = patch.field(
            "color",
            c.as_ref()
                .map_or(Value::Null, |c| c.to_ascii_lowercase().into()),
        );
    }
    if let Some(closed) = e.closed
        && closed != current.closed
    {
        let others = all
            .all()
            .iter()
            .filter(|s| s.id != id && s.closed == current.closed)
            .count();
        if others == 0 {
            return Err(Error::Invalid(format!(
                "{} is the only {} status; there must be at least one open and one closed status",
                current.name,
                if current.closed { "closed" } else { "open" },
            )));
        }
        patch = patch.field("closed", closed);
    }
    Ok(Status::from(&db.update(id, patch)?))
}

/// Move a status `step` places (-1 earlier, 1 later) among the statuses. At an end, nothing
/// changes.
pub fn move_by(db: &mut Db, id: i64, step: i32) -> Result<()> {
    let all = load(db)?;
    let Some(at) = all.all().iter().position(|s| s.id == id) else {
        return Err(Error::NotFound(id));
    };
    let Some(to) = usize::try_from(at as i32 + step)
        .ok()
        .filter(|i| *i < all.all().len())
    else {
        return Ok(());
    };
    // `move_item` inserts before the item at a position: before the neighbour to go earlier, and
    // just after it to go later.
    let neighbour = all.all()[to].position;
    db.move_item(
        id,
        None,
        Some(if step < 0 { neighbour } else { neighbour + 1 }),
    )?;
    Ok(())
}

/// Delete a status. What was in it moves to the first other status that is open (or closed, for
/// a closed one), so nothing is left without a status.
pub fn delete(db: &mut Db, id: i64) -> Result<()> {
    let all = load(db)?;
    let Some(gone) = all.get(id) else {
        return Err(Error::NotFound(id));
    };
    let Some(heir) = all
        .all()
        .iter()
        .find(|s| s.id != id && s.closed == gone.closed)
    else {
        return Err(Error::Invalid(format!(
            "{} is the only {} status; there must be at least one open and one closed status",
            gone.name,
            if gone.closed { "closed" } else { "open" },
        )));
    };
    let heir = heir.id;
    for kind in [TASK, EPIC] {
        let using = db.list(&Query {
            kind: Some(kind.into()),
            ..Query::default()
        })?;
        for item in using {
            if item.fields.get("status").and_then(Value::as_i64) == Some(id) {
                db.update(item.id, ItemPatch::default().field("status", heir))?;
            }
        }
    }
    db.delete(id)?;
    Ok(())
}
