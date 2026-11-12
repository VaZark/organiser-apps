//! `notes import-faves`: copy the lists and entries of a Faves database into folders and notes, so
//! what Faves held lives in Notes. Folders with the same name are reused and notes already in
//! them (by title) are skipped, so it can be run again safely.

use notes_schema::color::Color;
use notes_schema::lifecycle::{NewNote, create, create_folder, edit_folder};
use notes_schema::folder::FolderEdit;
use notes_schema::views::{Filter, Scope, folders, notes};
use organiser_cli_kit::Output;
use organiser_core::AppMeta;
use organiser_engine::{Db, Error, Field, FieldType, Kind, ParentFilter, Query, Result, Schema};
use serde_json::{Value, json};

/// Where Faves keeps its data.
const FAVES: AppMeta = AppMeta {
    id: "io.github.vazark.Faves",
    slug: "faves",
    name: "Faves",
    comment: "",
    categories: &[],
};

/// The part of Faves' schema the copy reads, so Notes does not depend on the Faves crates and the
/// Faves app can be removed.
static FAVES_SCHEMA: Schema = Schema {
    kinds: &[
        Kind {
            name: "list",
            fields: &[
                Field::new("notes", FieldType::Markdown),
                Field::new("rated", FieldType::Bool),
                Field::new("color", FieldType::Text),
            ],
        },
        Kind {
            name: "entry",
            fields: &[
                Field::new("notes", FieldType::Markdown),
                Field::new("favorite", FieldType::Bool),
                Field::new("color", FieldType::Text),
                Field::new("sticky", FieldType::Bool),
                Field::new("sticky_color", FieldType::Text),
                Field::new("rating", FieldType::Rating),
            ],
        },
    ],
};

fn text(v: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(String::from)
}

pub fn run(db: &mut Db, from: Option<&str>) -> Result<Output> {
    let path = from.map_or_else(
        || organiser_core::paths::db_path(&FAVES),
        std::path::PathBuf::from,
    );
    if !path.exists() {
        return Err(Error::Invalid(format!(
            "there is no Faves database at {}",
            path.display()
        )));
    }
    let faves = Db::open(&path, &FAVES_SCHEMA)?;
    // Entries of no list go in the default folder, as a note in no folder does.
    let home = notes_schema::lifecycle::ensure_default_folder(db)?;
    let mut made_folders = 0;
    let mut made_notes = 0;
    let mut skipped = 0;
    // Faves' list id -> the folder it became.
    let mut folder_of = std::collections::HashMap::new();
    let lists = faves.list(&Query {
        kind: Some("list".into()),
        sort: vec![organiser_engine::Sort::asc("position")],
        ..Query::default()
    })?;
    for l in &lists {
        let existing = folders(db)?
            .into_iter()
            .find(|s| s.folder.title == l.title)
            .map(|s| s.folder.id);
        let id = match existing {
            Some(id) => id,
            None => {
                let rated = l.fields.get("rated").and_then(Value::as_bool).unwrap_or(false);
                let f = create_folder(db, &l.title, rated)?;
                made_folders += 1;
                edit_folder(
                    db,
                    f.id,
                    &FolderEdit {
                        notes: text(&l.fields, "notes"),
                        color: text(&l.fields, "color")
                            .and_then(|c| Color::parse(&c))
                            .map(Some),
                        ..FolderEdit::default()
                    },
                )?;
                f.id
            }
        };
        folder_of.insert(l.id, id);
    }
    let entries = faves.list(&Query {
        kind: Some("entry".into()),
        parent: ParentFilter::Any,
        sort: vec![organiser_engine::Sort::asc("position")],
        ..Query::default()
    })?;
    for e in &entries {
        let folder = e.parent_id.and_then(|p| folder_of.get(&p).copied()).or(Some(home));
        // Already there (by title, in that folder): leave it.
        let scope = folder.map_or(Scope::Notes, Scope::Folder);
        let there = notes(
            db,
            &Filter {
                scope,
                include_archived: true,
                ..Filter::default()
            },
        )?
        .iter()
        .any(|n| n.title == e.title && n.folder_id == folder);
        if there {
            skipped += 1;
            continue;
        }
        let rating = e.fields.get("rating").and_then(Value::as_u64).map(|r| r as u8);
        let new = NewNote {
            title: e.title.clone(),
            body: text(&e.fields, "notes").unwrap_or_default(),
            color: text(&e.fields, "color").and_then(|c| Color::parse(&c)),
            tags: e.tags.clone(),
            folder,
            section: None,
            favorite: e.fields.get("favorite").and_then(Value::as_bool).unwrap_or(false),
            // Set below: it is kept even where the folder does not rate, as Faves kept it.
            rating: None,
        };
        let note = create(db, &new)?;
        if let Some(r) = rating {
            db.update(note.id, organiser_engine::ItemPatch::default().field("rating", r))?;
        }
        made_notes += 1;
    }
    Ok(Output::Custom {
        human: format!(
            "copied {made_notes} note{} and {made_folders} folder{} from {} ({skipped} already there)",
            if made_notes == 1 { "" } else { "s" },
            if made_folders == 1 { "" } else { "s" },
            path.display()
        ),
        json: json!({ "notes": made_notes, "folders": made_folders, "skipped": skipped }),
    })
}
