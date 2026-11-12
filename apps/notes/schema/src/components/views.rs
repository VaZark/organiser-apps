//! Reading notes: what each sidebar entry shows.

use organiser_engine::{
    Db, FieldFilter, Filters, Op, ParentFilter, Query, Result, Sort as Order, TagFilter,
};
use serde_json::Value;

use crate::primitives::folder::{Folder, FolderSummary};
use crate::primitives::section::{Section, SectionSummary};
use crate::primitives::note::Note;
use crate::{FOLDER, NOTE, SECTION};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Scope {
    /// Live notes in every folder, and in none: not archived, not trashed.
    #[default]
    Notes,
    /// The live notes marked as favourites.
    Favorites,
    /// The live notes filed in this folder.
    Folder(i64),
    Archive,
    Trash,
}

/// How the notes are ordered; pinned notes are always ahead of the rest.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Sort {
    /// The most recently changed first.
    #[default]
    Recent,
    /// The newest first.
    Created,
    Title,
    /// Best rated first; unrated last.
    Rating,
}

impl Sort {
    pub const ALL: [Sort; 4] = [Self::Recent, Self::Created, Self::Title, Self::Rating];

    pub fn label(self) -> &'static str {
        match self {
            Self::Recent => "Recently changed",
            Self::Created => "Newest",
            Self::Title => "Title",
            Self::Rating => "Best rated",
        }
    }
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
    /// What the person chose in the filters the schema offers (pinned, favourite, rating, …).
    pub fields: Filters,
    pub sort: Sort,
}

fn query(f: &Filter) -> Query {
    let mut q = Query {
        kind: Some(NOTE.into()),
        text: f.text.clone().filter(|t| !t.trim().is_empty()),
        tags: f.tags.clone(),
        // By title first for the rating, which has to be the effective one, and sorts after.
        sort: match f.sort {
            Sort::Recent => vec![Order::desc("updated")],
            Sort::Created => vec![Order::desc("created"), Order::desc("id")],
            Sort::Title | Sort::Rating => vec![Order::asc("title")],
        },
        ..Query::default()
    };
    // The live notes of every folder: a folder's notes are its children.
    let live = |q: &mut Query| {
        q.archived = Some(false);
        q.filters
            .push(FieldFilter::new("trashed", Op::IsNull, Value::Null));
    };
    match f.scope {
        Scope::Favorites => {
            live(&mut q);
            q.filters.push(FieldFilter::new("favorite", Op::Eq, true));
        }
        Scope::Folder(id) => {
            live(&mut q);
            q.parent = ParentFilter::Of(id);
        }
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

/// The folders whose notes are rated.
fn rated_folders(db: &Db) -> Result<std::collections::HashSet<i64>> {
    Ok(db
        .list(&Query {
            kind: Some(FOLDER.into()),
            ..Query::default()
        })?
        .iter()
        .map(Folder::from)
        .filter(|f| f.rated)
        .map(|f| f.id)
        .collect())
}

/// Notes in a scope, in the chosen order, with pinned notes ahead of the rest. A rating counts
/// only where the note's folder offers ratings: for a note in a folder that does not (or in none)
/// the stored rating is left out, so it is not shown, not matched by a rating filter and not used
/// to sort, though it is kept for when the folder rates again.
pub fn notes(db: &Db, f: &Filter) -> Result<Vec<Note>> {
    let rated = rated_folders(db)?;
    let mut v: Vec<Note> = db
        .list(&query(f))?
        .into_iter()
        .filter(|i| i.kind == NOTE)
        .map(|mut item| {
            if !item.parent_id.is_some_and(|p| rated.contains(&p)) {
                item.fields.remove("rating");
            }
            item
        })
        .filter(|i| f.fields.matches(i))
        .map(|i| Note::from(&i))
        .collect();
    // Stable, so the chosen order holds within each group.
    if f.sort == Sort::Rating {
        v.sort_by_key(|n| (!n.pinned, std::cmp::Reverse(n.rating)));
    } else {
        v.sort_by_key(|n| !n.pinned);
    }
    Ok(v)
}

/// Every folder, in order, with how many live notes it holds and how many are favourites.
pub fn folders(db: &Db) -> Result<Vec<FolderSummary>> {
    let found = db.list(&Query {
        kind: Some(FOLDER.into()),
        sort: vec![Order::asc("position")],
        ..Query::default()
    })?;
    let mut out = Vec::new();
    for item in &found {
        let live: Vec<Note> = notes(
            db,
            &Filter {
                scope: Scope::Folder(item.id),
                ..Filter::default()
            },
        )?;
        out.push(FolderSummary {
            folder: Folder::from(item),
            favorites: live.iter().filter(|n| n.favorite).count(),
            notes: live.len(),
        });
    }
    // The default folder, "Unassigned", comes last.
    out.sort_by_key(|s| s.folder.is_default);
    Ok(out)
}

/// The sections of a folder, in the person's order, with how many live notes each holds.
pub fn sections(db: &Db, folder: i64) -> Result<Vec<SectionSummary>> {
    let live = notes(
        db,
        &Filter {
            scope: Scope::Folder(folder),
            ..Filter::default()
        },
    )?;
    Ok(db
        .children(Some(folder))?
        .iter()
        .filter(|i| i.kind == SECTION)
        .map(Section::from)
        .map(|section| SectionSummary {
            notes: live.iter().filter(|n| n.section == Some(section.id)).count(),
            section,
        })
        .collect())
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
