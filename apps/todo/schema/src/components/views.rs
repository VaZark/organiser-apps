//! The lists a frontend shows.

use chrono::NaiveDate;
use organiser_engine::{
    Db, FieldFilter, Filters, Node, Op, Query, Result, Sort, TagFilter, build_tree,
};

use crate::primitives::sort::{SortKey, sort_tasks};
use crate::primitives::task::{Epic, EpicSummary, Section, SectionSummary, Task};
use crate::statuses;
use crate::{EPIC, SECTION, TASK};

/// The name of the status that makes a task current.
pub const IN_PROGRESS: &str = "In progress";

/// Which tasks to list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Scope {
    /// Every task, in the order the person arranged them.
    #[default]
    All,
    /// The tasks that are being worked on: those in the "In progress" status. Each task stands
    /// alone: nothing pulls in its parent or subtasks.
    Current,
    /// The archived tasks, as a tree like [`All`](Scope::All) (closed ones included).
    Archived,
}

#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub scope: Scope,
    /// Tasks in a closed status are hidden unless this is set.
    pub show_closed: bool,
    pub text: Option<String>,
    pub tags: TagFilter,
    pub epic: Option<i64>,
    /// Only the tasks that are in no epic.
    pub no_epic: bool,
    /// What the person chose in the filters the schema offers (status, priority, …).
    pub fields: Filters,
    pub include_archived: bool,
    /// Leave out subtasks: only top-level tasks are listed.
    pub hide_subtasks: bool,
    /// What to order by; pinned tasks are first whatever it is (see [`sort`](crate::sort)).
    pub sort: SortKey,
}

fn query(f: &Filter) -> Query {
    let mut q = Query {
        kind: Some(TASK.into()),
        text: f.text.clone(),
        tags: f.tags.clone(),
        archived: match f.scope {
            Scope::Archived => Some(true),
            _ => (!f.include_archived).then_some(false),
        },
        sort: vec![Sort::asc("position")],
        ..Query::default()
    };
    if let Some(e) = f.epic {
        q.filters.push(FieldFilter::new("epic", Op::Eq, e));
    } else if f.no_epic {
        q.filters
            .push(FieldFilter::new("epic", Op::IsNull, serde_json::Value::Null));
    }
    q
}

/// A forest read top to bottom: each task, then everything below it.
fn depth_first(nodes: Vec<Node<Task>>) -> Vec<Task> {
    let mut out = Vec::new();
    for n in nodes {
        out.push(n.value);
        out.extend(depth_first(n.children));
    }
    out
}

/// Tasks matching the filter, flat.
pub fn tasks(db: &Db, f: &Filter, _today: NaiveDate) -> Result<Vec<Task>> {
    let items = db.list(&query(f))?;
    // How many subtasks each task has, closed ones too: a tie-breaker of the sort.
    let mut subs: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
    for i in &items {
        if let Some(p) = i.parent_id {
            *subs.entry(p).or_default() += 1;
        }
    }
    // Which tasks are finished is the person's to say, and a task may have no status at all, so
    // closed ones are left out here rather than by an engine filter (which would drop those).
    let all = statuses::load(db)?;
    // Choosing statuses is asking for them, closed ones included.
    let chose_status = !f.fields.any_of("status").is_empty();
    let found = items
        .iter()
        .filter(|i| f.fields.matches(i))
        .map(Task::from)
        .filter(|t| !f.hide_subtasks || t.parent_id.is_none())
        .filter(|t| {
            f.show_closed || chose_status || f.scope == Scope::Archived || all.is_open(t.status)
        });
    // "In progress" is the person's own status, found by its name, so it is applied here.
    let in_progress = all.by_name(IN_PROGRESS).map(|s| s.id);
    Ok(match f.scope {
        // Parents before their subtasks, siblings in the person's order: not a raw sort by
        // `position`, which only orders siblings.
        // Sorted, pinned first, then built into the tree: siblings keep the order they are in.
        Scope::All | Scope::Archived => {
            let mut all: Vec<Task> = found.collect();
            sort_tasks(&mut all, f.sort, &subs);
            depth_first(build_tree(all, |t| t.id, |t| t.parent_id))
        }
        Scope::Current => {
            let mut current: Vec<Task> = found
                .filter(|t| in_progress.is_some() && all.resolve(t.status).map(|s| s.id) == in_progress)
                .collect();
            // Pinned first, then as the other views are sorted; the sort is stable, so ties stay
            // in the person's own order.
            sort_tasks(&mut current, f.sort, &subs);
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

/// Epics with how many of their tasks are still open. An epic has no status of its own: its
/// progress is how far its tasks are.
pub fn epics(db: &Db) -> Result<Vec<EpicSummary>> {
    let q = Query {
        kind: Some(EPIC.into()),
        archived: Some(false),
        sort: vec![Sort::asc("position")],
        ..Query::default()
    };
    let all = statuses::load(db)?;
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
            .filter(|t| all.is_open(Task::from(*t).status))
            .count();
        out.push(EpicSummary {
            epic: Epic::from(&item),
            open,
            total: linked.len(),
        });
    }
    Ok(out)
}

/// How many tasks are in no epic still open, and in all (archived ones left out).
pub fn unassigned(db: &Db) -> Result<(usize, usize)> {
    let all = statuses::load(db)?;
    let tasks: Vec<Task> = db
        .list(&Query {
            kind: Some(TASK.into()),
            archived: Some(false),
            filters: vec![FieldFilter::new("epic", Op::IsNull, serde_json::Value::Null)],
            ..Query::default()
        })?
        .iter()
        .map(Task::from)
        .collect();
    let open = tasks.iter().filter(|t| all.is_open(t.status)).count();
    Ok((open, tasks.len()))
}

/// The sections of an epic, in the person's order, with how many of their tasks are open.
pub fn sections(db: &Db, epic: i64) -> Result<Vec<SectionSummary>> {
    let all = statuses::load(db)?;
    let tasks: Vec<Task> = db
        .list(&Query {
            kind: Some(TASK.into()),
            archived: Some(false),
            filters: vec![FieldFilter::new("epic", Op::Eq, epic)],
            ..Query::default()
        })?
        .iter()
        .map(Task::from)
        .collect();
    Ok(db
        .children(Some(epic))?
        .iter()
        .filter(|i| i.kind == SECTION)
        .map(Section::from)
        .map(|section| {
            let mine = tasks.iter().filter(|t| t.section == Some(section.id));
            let total = mine.clone().count();
            let open = mine.filter(|t| all.is_open(t.status)).count();
            SectionSummary {
                section,
                open,
                total,
            }
        })
        .collect())
}

pub fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}
