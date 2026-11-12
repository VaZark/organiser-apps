//! # notes-schema
//!
//! The Notes app (Keep-style cards: custom colours, pinning, archive, trash, tags) on the common
//! engine. Layers, as in the engine (`tests/layers.rs` enforces them):
//!
//! * [`primitives`]: pure note model and colour rules.
//! * [`components`]: operations that need a [`organiser_engine::Db`]: creating, trashing, listing.

pub mod components;
pub mod primitives;

use organiser_core::AppMeta;
use organiser_core::sticky::{StickyKind, StickySpec};
use organiser_engine::{Field, FieldType, Kind, Schema};

pub use components::{lifecycle, views};
pub use organiser_markdown::{doc, export as md_export, parse as md_parse, tasks as md_tasks};
pub use primitives::{color, md_note, note, quick};

pub const META: AppMeta = AppMeta {
    id: "io.github.vazark.Notes",
    slug: "notes",
    name: "Notes",
    comment: "Capture notes as colourful cards",
    categories: &["Office", "Utility"],
};

pub const NOTE: &str = "note";

/// What this app offers Sticky Notes: its notes can be desktop stickies.
pub const STICKY: StickySpec = StickySpec {
    meta: META,
    kinds: &[StickyKind {
        kind: NOTE,
        body: "body",
    }],
};

pub static SCHEMA: Schema = Schema {
    kinds: &[Kind {
        name: NOTE,
        fields: &[
            Field::new("body", FieldType::Markdown).searchable(),
            // Any `#rrggbb`; see `color::Color`. Absent means the default card.
            Field::new("color", FieldType::Text),
            Field::new("pinned", FieldType::Bool).indexed(),
            // A desktop sticky (see organiser-sticky); `sticky_color` is `#rrggbb`.
            Field::new("sticky", FieldType::Bool).indexed().internal(),
            Field::new("sticky_color", FieldType::Text).internal(),
            // Unix seconds when moved to the trash; absent means not trashed.
            Field::new("trashed", FieldType::Number)
                .indexed()
                .internal(),
        ],
    }],
};
