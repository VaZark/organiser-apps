//! Reading notes: what each sidebar entry shows.

use organiser_engine::{Db, FieldFilter, Filters, Op, Query, Result, Sort, TagFilter};
use serde_json::Value;

use crate::NOTE;
use crate::primitives::note::Note;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Scope {
    /// Live notes: not archived, not trashed.
    #[default]
    Notes,
    Archive,
    Trash,
}

#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub scope: Scope,
    pub text: Option<String>,
    /// Tags, as the engine takes them: groups of tags that are alternatives (see
    /// `TagFilter::blocks`), or simply every tag in `all`.
    pub tags: TagFilter,
    /// Search also looks in the archive (never the trash).
    pub include_archived: bool,
    /// What the person chose in the filters the schema offers (pinned, …).
    pub fields: Filters,
}

fn query(f: &Filter) -> Query {
    let mut q = Query {
        kind: Some(NOTE.into()),
        text: f.text.clone().filter(|t| !t.trim().is_empty()),
        tags: f.tags.clone(),
        sort: vec![Sort::desc("updated")],
        ..Query::default()
    };
    match f.scope {
        Scope::Notes => {
            q.archived = if f.include_archived {
                None
            } else {
                Some(false)
            };
            q.filters
                .push(FieldFilter::new("trashed", Op::IsNull, Value::Null));
        }
        Scope::Archive => {
            q.archived = Some(true);
            q.filters
                .push(FieldFilter::new("trashed", Op::IsNull, Value::Null));
        }
        Scope::Trash => q
            .filters
            .push(FieldFilter::new("trashed", Op::NotNull, Value::Null)),
    }
    q
}

/// Notes in a scope, newest first, with pinned notes ahead of the rest.
pub fn notes(db: &Db, f: &Filter) -> Result<Vec<Note>> {
    let mut v: Vec<Note> = db
        .list(&query(f))?
        .iter()
        .filter(|i| f.fields.matches(i))
        .map(Note::from)
        .collect();
    // Stable, so "newest first" holds within each group.
    v.sort_by_key(|n| !n.pinned);
    Ok(v)
}

/// Everything in the trash, most recently trashed first.
pub fn trashed(db: &Db) -> Result<Vec<Note>> {
    let mut v = notes(
        db,
        &Filter {
            scope: Scope::Trash,
            ..Filter::default()
        },
    )?;
    v.sort_by_key(|n| std::cmp::Reverse(n.trashed));
    Ok(v)
}
