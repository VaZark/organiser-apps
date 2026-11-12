//! Creating, editing and closing tasks and epics, with GTG's rules for subtasks: closing a task
//! closes its open subtasks; reopening one reopens the closed tasks above it.

use chrono::NaiveDate;
use organiser_engine::{Db, Error, FieldFilter, ItemPatch, NewItem, Op, Query, Result};

use crate::primitives::priority::Priority;
use crate::primitives::task::{Edit, Epic, Section, Task};
use crate::statuses;
use crate::{EPIC, SECTION, TASK};

#[derive(Debug, Clone, Default)]
pub struct NewTask {
    pub title: String,
    /// Markdown.
    pub notes: String,
    pub parent: Option<i64>,
    pub epic: Option<i64>,
    /// A section of that epic; ignored without an epic.
    pub section: Option<i64>,
    pub start: Option<NaiveDate>,
    pub due: Option<NaiveDate>,
    pub priority: Option<Priority>,
    /// Where it starts; the first open status when not given.
    pub status: Option<i64>,
    pub tags: Vec<String>,
    pub pinned: bool,
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
    if t.pinned {
        new = new.field("pinned", true);
    }
    if let Some(e) = t.epic {
        new = new.field("epic", e);
        if let Some(sec) = t.section {
            check_section(db, sec, e)?;
            new = new.field("section", sec);
        }
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

/// The section must be one of that epic's.
fn check_section(db: &Db, section: i64, epic: i64) -> Result<()> {
    let item = db.get(section)?;
    if item.kind != SECTION || item.parent_id != Some(epic) {
        return Err(Error::Invalid(format!(
            "item {section} is not a section of epic {epic}"
        )));
    }
    Ok(())
}

/// Edit a task. A section always belongs to the task's epic: changing or clearing the epic
/// clears the section, unless the same edit names one of the new epic's. When the epic or the
/// section changes, the task's subtasks follow.
pub fn edit(db: &mut Db, id: i64, edit: &Edit) -> Result<Task> {
    let mut edit = edit.clone();
    let epic = match edit.epic {
        Some(e) => e,
        None => Task::from(&db.get(id)?).epic,
    };
    match (edit.section, epic) {
        (Some(Some(sec)), Some(e)) => check_section(db, sec, e)?,
        (Some(Some(_)), None) => {
            return Err(Error::Invalid("a section needs an epic".into()));
        }
        (None, _) if edit.epic.is_some() && Task::from(&db.get(id)?).epic != epic => {
            edit.section = Some(None);
        }
        _ => {}
    }
    let task = Task::from(&db.update(id, edit.to_patch())?);
    // A task and its subtasks live in one place: they follow it to its new epic and section.
    if edit.epic.is_some() || edit.section.is_some() {
        let place = |v: Option<i64>| v.map_or(serde_json::Value::Null, serde_json::Value::from);
        for sub in descendants(db, id)? {
            let patch = ItemPatch::default()
                .field("epic", place(task.epic))
                .field("section", place(task.section));
            db.update(sub.id, patch)?;
        }
    }
    Ok(task)
}

/// Pin a task, to keep it ahead of the others in every view, or unpin it.
pub fn set_pinned(db: &mut Db, id: i64, pinned: bool) -> Result<Task> {
    let v = if pinned {
        serde_json::Value::Bool(true)
    } else {
        serde_json::Value::Null
    };
    Ok(Task::from(
        &db.update(id, ItemPatch::default().field("pinned", v))?,
    ))
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

/// Move a task under another task (`Some`), making it a subtask, or out to the top level (`None`).
///
/// Under a parent it joins that parent's epic and section, and so do its own subtasks: a task
/// and its subtasks live in one place. Out to the top level it keeps its epic and section.
pub fn reparent(db: &mut Db, id: i64, parent: Option<i64>) -> Result<Task> {
    if db.get(id)?.kind != TASK {
        return Err(Error::Invalid(format!("item {id} is not a task")));
    }
    if let Some(p) = parent {
        let target = db.get(p)?;
        if target.kind != TASK {
            return Err(Error::Invalid(format!("item {p} is not a task")));
        }
        if p == id || descendants(db, id)?.iter().any(|t| t.id == p) {
            return Err(Error::Invalid(
                "a task cannot become a subtask of itself or of its own subtasks".into(),
            ));
        }
        let home = Task::from(&target);
        // Its subtasks follow it (see `edit`).
        edit(
            db,
            id,
            &Edit {
                epic: Some(home.epic),
                section: Some(home.section),
                ..Edit::default()
            },
        )?;
    }
    Ok(Task::from(&db.move_item(id, parent, None)?))
}

// ---- epics ---------------------------------------------------------------------------------

pub fn create_epic(db: &mut Db, title: &str) -> Result<Epic> {
    Ok(Epic::from(&db.create(NewItem::new(EPIC, title))?))
}

pub fn edit_epic(db: &mut Db, id: i64, edit: &Edit) -> Result<Epic> {
    Ok(Epic::from(&db.update(id, edit.to_patch())?))
}

/// Delete an epic. The engine unlinks its tasks, which stay, no longer in any epic.
pub fn delete_epic(db: &mut Db, id: i64) -> Result<()> {
    if db.get(id)?.kind != EPIC {
        return Err(Error::Invalid(format!("item {id} is not an epic")));
    }
    db.delete(id)?;
    Ok(())
}

/// Delete an epic and every task in it, subtasks included. Returns how many tasks went.
pub fn delete_epic_and_tasks(db: &mut Db, id: i64) -> Result<usize> {
    if db.get(id)?.kind != EPIC {
        return Err(Error::Invalid(format!("item {id} is not an epic")));
    }
    let linked = db.list(&Query {
        kind: Some(TASK.into()),
        filters: vec![FieldFilter::new("epic", Op::Eq, id)],
        ..Query::default()
    })?;
    let mut gone = 0;
    for t in linked {
        // A subtask goes with its parent, which may already have taken it.
        match db.delete(t.id) {
            Ok(n) => gone += n,
            Err(Error::NotFound(_)) => {}
            Err(e) => return Err(e),
        }
    }
    db.delete(id)?;
    Ok(gone)
}

// ---- sections ------------------------------------------------------------------------------

/// Add a section, last, to an epic.
pub fn create_section(db: &mut Db, epic: i64, title: &str) -> Result<Section> {
    if db.get(epic)?.kind != EPIC {
        return Err(Error::Invalid(format!("item {epic} is not an epic")));
    }
    let mut new = NewItem::new(SECTION, title);
    new.parent_id = Some(epic);
    Ok(Section::from(&db.create(new)?))
}

/// Edit a section's title, description (`notes`) or colour.
pub fn edit_section(db: &mut Db, id: i64, edit: &Edit) -> Result<Section> {
    if db.get(id)?.kind != SECTION {
        return Err(Error::Invalid(format!("item {id} is not a section")));
    }
    Ok(Section::from(&db.update(id, edit.to_patch())?))
}

pub fn rename_section(db: &mut Db, id: i64, title: &str) -> Result<Section> {
    Ok(Section::from(&db.update(
        id,
        ItemPatch {
            title: Some(title.into()),
            ..ItemPatch::default()
        },
    )?))
}

/// Move a section to place `to` (0 is first) among its epic's sections, the others keeping their
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

/// Delete a section. Its tasks stay in the epic, in no section.
pub fn delete_section(db: &mut Db, id: i64) -> Result<()> {
    if db.get(id)?.kind != SECTION {
        return Err(Error::Invalid(format!("item {id} is not a section")));
    }
    db.delete(id)?;
    Ok(())
}

/// Move an epic before `before` (another epic), or last when there is none: the others keep
/// their order.
pub fn move_epic(db: &mut Db, id: i64, before: Option<i64>) -> Result<()> {
    if db.get(id)?.kind != EPIC {
        return Err(Error::Invalid(format!("item {id} is not an epic")));
    }
    match before.filter(|b| *b != id) {
        Some(b) => {
            let next = db.get(b)?;
            if next.kind != EPIC {
                return Err(Error::Invalid(format!("item {b} is not an epic")));
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
