//! # notes-schema
//!
//! The Notes app (Keep-style cards: custom colours, pinning, archive, trash, tags, and folders with
//! favourites and ratings) on the common engine. Layers, as in the engine (`tests/layers.rs` enforces them):
//!
//! * [`primitives`]: pure note model and colour rules.
//! * [`components`]: operations that need a [`organiser_engine::Db`]: creating, trashing, listing.

pub mod components;
pub mod primitives;

use organiser_core::AppMeta;
use organiser_core::sticky::{StickyKind, StickySpec};
use organiser_engine::{Field, FieldType, Kind, Schema};

pub use components::{lifecycle, vault, views};
pub use organiser_markdown::{doc, export as md_export, parse as md_parse, tasks as md_tasks};
pub use organiser_vault::front_matter;
pub use primitives::{color, folder, md_note, note, quick};

pub const META: AppMeta = AppMeta {
    id: "io.github.vazark.Notes",
    slug: "notes",
    name: "Notes",
    comment: "Capture notes as colourful cards",
    categories: &["Office", "Utility"],
};

pub const NOTE: &str = "note";
pub const FOLDER: &str = "folder";
/// What a folder's sections used to be: a kind kept only so a database that still has some loads;
/// [`lifecycle::migrate_sections`] turns each into a folder inside its folder.
pub const SECTION: &str = "section";

/// What this app offers Sticky Notes: its notes can be desktop stickies.
pub const STICKY: StickySpec = StickySpec {
    meta: META,
    kinds: &[StickyKind {
        kind: NOTE,
        body: "body",
    }],
};

pub static SCHEMA: Schema = Schema {
    kinds: &[
        Kind {
            name: FOLDER,
            fields: &[
                Field::new("notes", FieldType::Markdown).searchable(),
                // Notes in this folder show a rating, and it counts in searching and sorting.
                // Switching it off keeps the ratings; they are just not shown or used.
                Field::new("rated", FieldType::Bool),
                // `#rrggbb`: the folder's colour, on its icon and its notes without their own.
                Field::new("color", FieldType::Text),
                // The name of its icon in the catalogue (`organiser_ui::components::icons`); none
                // is the folder. Only the name is stored.
                Field::new("icon", FieldType::Text),
                // The one folder every note without another is in: "Unassigned". It cannot be
                // deleted (see `lifecycle::ensure_default_folder`).
                Field::new("default", FieldType::Bool).indexed().internal(),
            ],
        },
        Kind {
            name: SECTION,
            fields: &[
                // What the section is for.
                Field::new("notes", FieldType::Markdown).searchable(),
                // `#rrggbb`.
                Field::new("color", FieldType::Text),
            ],
        },
        Kind {
        name: NOTE,
        fields: &[
            Field::new("body", FieldType::Markdown).searchable(),
            Field::new("favorite", FieldType::Bool).indexed(),
            // Kept when its folder stops rating: see `views::notes`, which leaves out the rating
            // of a note whose folder does not offer one.
            Field::new("rating", FieldType::Rating).indexed(),
            // Legacy: the section it was in. Only read by `lifecycle::migrate_sections`, which
            // turns a section into a folder and clears this.
            Field::new("section", FieldType::Reference(SECTION))
                .indexed()
                .internal(),
            // Any `#rrggbb`; see `color::Color`. Absent means the default card.
            Field::new("color", FieldType::Text),
            Field::new("pinned", FieldType::Bool).indexed(),
            // A desktop sticky (see organiser-sticky); `sticky_color` is `#rrggbb`.
            Field::new("sticky", FieldType::Bool).indexed().internal(),
            Field::new("sticky_color", FieldType::Text).internal(),
            // The vault (see `components::vault`): the note's file, relative to the vault, a hash of
            // that file as last read or written, and of the file this note would be written as then.
            Field::new("file", FieldType::Text).internal(),
            Field::new("file_hash", FieldType::Text).internal(),
            Field::new("file_sig", FieldType::Text).internal(),
            // Unix seconds when moved to the trash; absent means not trashed.
            Field::new("trashed", FieldType::Number)
                .indexed()
                .internal(),
        ],
        },
    ],
};
