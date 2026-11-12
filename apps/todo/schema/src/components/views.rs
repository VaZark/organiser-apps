//! The lists a frontend shows.

use chrono::NaiveDate;
use organiser_engine::{
    Db, FieldFilter, Filters, Node, Op, Query, Result, Sort, TagFilter, build_tree,
};

use crate::primitives::sort::{SortKey, sort_tasks};
use crate::primitives::task::{Epic, EpicSummary, Section, SectionSummary, Task};
use crate::statuses;
use crate::{EPIC, SECTION, TASK};

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
    /// Tasks in a closed status are hidden unless this is set.
    pub show_closed: bool,
    pub text: Option<String>,
    pub tags: TagFilter,
    pub epic: Option<i64>,
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
        archived: (!f.include_archived).then_some(false),
        sort: vec![Sort::asc("position")],
        ..Query::default()
    };
    if let Some(e) = f.epic {
        q.filters.push(FieldFilter::new("epic", Op::Eq, e));
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
pub fn tasks(db: &Db, f: &Filter, today: NaiveDate) -> Result<Vec<Task>> {
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
        .filter(|t| f.show_closed || chose_status || all.is_open(t.status));
    // "Current" is an OR over two nullable dates, which the engine's AND-only filters cannot
    // say, so it is applied here.
    Ok(match f.scope {
        // Parents before their subtasks, siblings in the person's order: not a raw sort by
        // `position`, which only orders siblings.
        // Sorted, pinned first, then built into the tree: siblings keep the order they are in.
        Scope::All => {
            let mut all: Vec<Task> = found.collect();
            sort_tasks(&mut all, f.sort, &subs);
            depth_first(build_tree(all, |t| t.id, |t| t.parent_id))
        }
        Scope::Current => {
            let mut current: Vec<Task> = found.filter(|t| t.is_current(today)).collect();
            // Pinned first; then dated deadlines, soonest first; then those that only have a start. The sort
            // is stable, so ties stay in the person's own order.
            if f.sort == SortKey::Manual {
                current.sort_by_key(|t| (!t.pinned, t.due.is_none(), t.due, t.start));
            } else {
                sort_tasks(&mut current, f.sort, &subs);
            }
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
    let q = Query {
        kind: Some(EPIC.into()),
        archived: Some(false),
        sort: vec![Sort::asc("position")],
        ..Query::default()
    };
    let all = statuses::load(db)?;
    let mut out = Vec::new();
    for item in db.list(&q)? {
        if !show_closed && all.is_closed(Epic::from(&item).status) {
            continue;
        }
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
