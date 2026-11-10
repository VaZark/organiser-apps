//! The lists a frontend shows.

use chrono::NaiveDate;
use organiser_engine::{Db, FieldFilter, Node, Op, Query, Result, Sort, TagFilter, build_tree};

use crate::primitives::status::Status;
use crate::primitives::task::{Epic, EpicSummary, Task};
use crate::{EPIC, TASK};

/// Which tasks to list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Scope {
    /// Every task, in the order the person arranged them.
    #[default]
    All,
    /// Scheduled tasks whose start-to-due span includes today (see [`Task::is_current`]), the
    /// nearest deadline first. Each task stands alone: nothing pulls in its parent or subtasks.
    Current,
}

#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub scope: Scope,
    /// Done and won't-do tasks are hidden unless this is set.
    pub show_closed: bool,
    pub text: Option<String>,
    pub tags: TagFilter,
    pub epic: Option<i64>,
    pub include_archived: bool,
}

fn query(f: &Filter) -> Query {
    let mut q = Query {
        kind: Some(TASK.into()),
        text: f.text.clone(),
        tags: f.tags.clone(),
        archived: (!f.include_archived).then_some(false),
        sort: vec![Sort::asc("position")],
        ..Query::default()
    };
    if !f.show_closed {
        q.filters
            .push(FieldFilter::new("status", Op::Ne, Status::Done.as_str()));
        q.filters.push(FieldFilter::new(
            "status",
            Op::Ne,
            Status::Dismissed.as_str(),
        ));
    }
    if let Some(e) = f.epic {
        q.filters.push(FieldFilter::new("epic", Op::Eq, e));
    }
    q
}

/// Tasks matching the filter, flat.
pub fn tasks(db: &Db, f: &Filter, today: NaiveDate) -> Result<Vec<Task>> {
    let items = db.list(&query(f))?;
    let found = items.iter().map(Task::from);
    // "Current" is an OR over two nullable dates, which the engine's AND-only filters cannot
    // say, so it is applied here.
    Ok(match f.scope {
        Scope::All => found.collect(),
        Scope::Current => {
            let mut current: Vec<Task> = found.filter(|t| t.is_current(today)).collect();
            // Dated deadlines first, soonest first; then those that only have a start. The sort
            // is stable, so ties stay in the person's own order.
            current.sort_by_key(|t| (t.due.is_none(), t.due, t.start));
            current
        }
    })
}

/// The same tasks as a forest: a task whose parent did not match becomes a root.
pub fn forest(db: &Db, f: &Filter, today: NaiveDate) -> Result<Vec<Node<Task>>> {
    Ok(build_tree(tasks(db, f, today)?, |t| t.id, |t| t.parent_id))
}

/// The direct subtasks of a task, closed ones included.
pub fn subtasks(db: &Db, id: i64) -> Result<Vec<Task>> {
    Ok(db.children(Some(id))?.iter().map(Task::from).collect())
}

/// Epics with how many of their tasks are still open. Closed epics only with `show_closed`.
pub fn epics(db: &Db, show_closed: bool) -> Result<Vec<EpicSummary>> {
    let mut q = Query {
        kind: Some(EPIC.into()),
        archived: Some(false),
        sort: vec![Sort::asc("position")],
        ..Query::default()
    };
    if !show_closed {
        q.filters
            .push(FieldFilter::new("status", Op::Ne, Status::Done.as_str()));
        q.filters.push(FieldFilter::new(
            "status",
            Op::Ne,
            Status::Dismissed.as_str(),
        ));
    }
    let mut out = Vec::new();
    for item in db.list(&q)? {
        let linked = db.list(&Query {
            kind: Some(TASK.into()),
            archived: Some(false),
            filters: vec![FieldFilter::new("epic", Op::Eq, item.id)],
            ..Query::default()
        })?;
        let open = linked
            .iter()
            .filter(|t| Task::from(*t).status.is_open())
            .count();
        out.push(EpicSummary {
            epic: Epic::from(&item),
            open,
            total: linked.len(),
        });
    }
    Ok(out)
}

pub fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}
