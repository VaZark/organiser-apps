//! The lists a frontend shows.

use organiser_engine::{
    Db, FieldFilter, Op, ParentFilter, Query, Result, Sort as Order, TagFilter,
};

use crate::primitives::model::{Entry, List, ListSummary};
use crate::{ENTRY, LIST};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Sort {
    /// The order the person arranged.
    #[default]
    Manual,
    Title,
    /// Best rated first; unrated last.
    Rating,
    /// Newest first.
    Recent,
}

#[derive(Debug, Clone, Default)]
pub struct Filter {
    /// One list, or every list.
    pub list: Option<i64>,
    pub text: Option<String>,
    pub tags: TagFilter,
    pub favorites: bool,
    /// At least this many stars. Unrated entries do not match.
    pub min_rating: Option<u8>,
    /// What the person chose in the filters the schema offers (favourite, rating).
    pub fields: organiser_engine::Filters,
    pub sort: Sort,
}

/// Every list with its counts, in order.
pub fn lists(db: &Db) -> Result<Vec<ListSummary>> {
    let found = db.list(&Query {
        kind: Some(LIST.into()),
        sort: vec![Order::asc("position")],
        ..Query::default()
    })?;
    let mut out = Vec::new();
    for item in &found {
        let entries: Vec<Entry> = db
            .list(&Query {
                kind: Some(ENTRY.into()),
                parent: ParentFilter::Of(item.id),
                ..Query::default()
            })?
            .iter()
            .map(Entry::from)
            .collect();
        out.push(ListSummary {
            list: List::from(item),
            favorites: entries.iter().filter(|e| e.favorite).count(),
            entries: entries.len(),
        });
    }
    Ok(out)
}

pub fn entries(db: &Db, f: &Filter) -> Result<Vec<Entry>> {
    let mut q = Query {
        kind: Some(ENTRY.into()),
        parent: f.list.map_or(ParentFilter::Any, ParentFilter::Of),
        text: f.text.clone(),
        tags: f.tags.clone(),
        sort: match f.sort {
            Sort::Manual => vec![Order::asc("position")],
            Sort::Title => vec![Order::asc("title")],
            Sort::Rating => vec![Order::desc("rating"), Order::asc("title")],
            Sort::Recent => vec![Order::desc("created"), Order::desc("id")],
        },
        ..Query::default()
    };
    if f.favorites {
        q.filters.push(FieldFilter::new("favorite", Op::Eq, true));
    }
    if let Some(n) = f.min_rating {
        q.filters.push(FieldFilter::new("rating", Op::Ge, n));
    }
    Ok(db
        .list(&q)?
        .iter()
        .filter(|i| f.fields.matches(i))
        .map(Entry::from)
        .collect())
}
