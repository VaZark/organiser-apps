//! Creating, editing and closing tasks and epics, with GTG's rules for subtasks: closing a task
//! closes its open subtasks; reopening one reopens the closed tasks above it.

use chrono::NaiveDate;
use organiser_engine::{Db, Error, ItemPatch, NewItem, Result};

use crate::primitives::priority::Priority;
use crate::primitives::task::{Edit, Epic, Task};
use crate::statuses;
use crate::{EPIC, TASK};

#[derive(Debug, Clone, Default)]
pub struct NewTask {
    pub title: String,
    /// Markdown.
    pub notes: String,
    pub parent: Option<i64>,
    pub epic: Option<i64>,
    pub start: Option<NaiveDate>,
    pub due: Option<NaiveDate>,
    pub priority: Option<Priority>,
    /// Where it starts; the first open status when not given.
    pub status: Option<i64>,
    pub tags: Vec<String>,
}

impl NewTask {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.into(),
            ..Self::default()
        }
    }
}

pub fn create(db: &mut Db, t: &NewTask) -> Result<Task> {
    let mut new = NewItem::new(TASK, &t.title);
    new.parent_id = t.parent;
    new.tags = t.tags.clone();
    if !t.notes.is_empty() {
        new = new.field("notes", t.notes.as_str());
    }
    if let Some(d) = t.start {
        new = new.field("start", d.to_string());
    }
    if let Some(d) = t.due {
        new = new.field("due", d.to_string());
    }
    if let Some(p) = t.priority {
        new = new.field("priority", p.as_str());
    }
    if let Some(e) = t.epic {
        new = new.field("epic", e);
    }
    let start = match t.status {
        Some(s) => Some(s),
        None => statuses::load(db)?.first_open().map(|s| s.id),
    };
    if let Some(s) = start {
        new = new.field("status", s);
    }
    Ok(Task::from(&db.create(new)?))
}

pub fn edit(db: &mut Db, id: i64, edit: &Edit) -> Result<Task> {
    Ok(Task::from(&db.update(id, edit.to_patch())?))
}

/// Make the item's tags exactly `tags` (adding and removing only the difference).
pub fn set_tags(db: &mut Db, id: i64, tags: &[String]) -> Result<()> {
    db.set_tags(id, tags).map(|_| ())
}

/// Set a task's status. Returns every task whose status changed.
///
/// Finishing a task (moving it to a closed status) moves its unfinished subtasks there too;
/// reopening one moves the finished tasks above it into the same open status.
pub fn set_status(db: &mut Db, id: i64, status: i64) -> Result<Vec<Task>> {
    let all = statuses::load(db)?;
    let Some(target) = all.get(status) else {
        return Err(Error::Invalid(format!("there is no status {status}")));
    };
    let item = db.get(id)?;
    if item.kind != TASK {
        return Err(Error::Invalid(format!(
            "item {id} is a {}, not a task",
            item.kind
        )));
    }
    let current = Task::from(&item);
    let mut changed = Vec::new();
    if current.status == Some(status) {
        return Ok(changed);
    }
    let mut apply = |db: &mut Db, id: i64| -> Result<()> {
        let item = db.update(id, ItemPatch::default().field("status", status))?;
        changed.push(Task::from(&item));
        Ok(())
    };
    if target.closed {
        for sub in descendants(db, id)? {
            if all.is_open(sub.status) {
                apply(db, sub.id)?;
            }
        }
        apply(db, id)?;
    } else {
        apply(db, id)?;
        let mut up = current.parent_id;
        while let Some(pid) = up {
            let p = Task::from(&db.get(pid)?);
            if all.is_closed(p.status) {
                apply(db, pid)?;
            }
            up = p.parent_id;
        }
    }
    Ok(changed)
}

/// Everything below a task, depth first.
pub fn descendants(db: &Db, id: i64) -> Result<Vec<Task>> {
    let mut out = Vec::new();
    for child in db.children(Some(id))? {
        let t = Task::from(&child);
        let below = descendants(db, t.id)?;
        out.push(t);
        out.extend(below);
    }
    Ok(out)
}

// ---- epics ---------------------------------------------------------------------------------

pub fn create_epic(db: &mut Db, title: &str) -> Result<Epic> {
    Ok(Epic::from(&db.create(NewItem::new(EPIC, title))?))
}

pub fn edit_epic(db: &mut Db, id: i64, edit: &Edit) -> Result<Epic> {
    Ok(Epic::from(&db.update(id, edit.to_patch())?))
}

/// Closing an epic leaves its tasks alone: they may belong to follow-up work.
pub fn set_epic_status(db: &mut Db, id: i64, status: i64) -> Result<Epic> {
    if statuses::load(db)?.get(status).is_none() {
        return Err(Error::Invalid(format!("there is no status {status}")));
    }
    Ok(Epic::from(&db.update(
        id,
        ItemPatch::default().field("status", status),
    )?))
}

/// Delete an epic. The engine unlinks its tasks, which stay, no longer in any epic.
pub fn delete_epic(db: &mut Db, id: i64) -> Result<()> {
    if db.get(id)?.kind != EPIC {
        return Err(Error::Invalid(format!("item {id} is not an epic")));
    }
    db.delete(id)?;
    Ok(())
}
