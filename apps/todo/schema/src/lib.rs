//! # todo-schema
//!
//! The Todo app (GTG-style: nested tasks, start and due dates, tags) expressed on the common
//! engine. Layers, as in the engine (`tests/layers.rs` enforces them):
//!
//! * [`primitives`]: pure task and epic model and rules: status, priority, dates, quick add.
//! * [`components`]: operations that need a [`organiser_engine::Db`]: creating, closing, listing.

pub mod components;
pub mod primitives;

use organiser_core::AppMeta;
use organiser_engine::{Field, FieldType, Kind, Schema};

pub use components::{lifecycle, views};
pub use primitives::{board, due, priority, quick, status, task};

pub const META: AppMeta = AppMeta {
    id: "io.github.vazark.Todo",
    slug: "todo",
    name: "Todo",
    comment: "Organise tasks, subtasks and deadlines",
    categories: &["Office", "ProjectManagement"],
};

pub const TASK: &str = "task";
pub const EPIC: &str = "epic";

pub static SCHEMA: Schema = Schema {
    kinds: &[
        Kind {
            name: TASK,
            fields: &[
                Field::new("notes", FieldType::Markdown).searchable(),
                Field::new("status", FieldType::Enum(status::NAMES))
                    .indexed()
                    .default_to("open"),
                Field::new("priority", FieldType::Enum(priority::NAMES))
                    .indexed()
                    .default_to("medium"),
                Field::new("start", FieldType::Date).indexed(),
                Field::new("due", FieldType::Date).indexed(),
                Field::new("epic", FieldType::Reference(EPIC)).indexed(),
            ],
        },
        Kind {
            name: EPIC,
            fields: &[
                Field::new("notes", FieldType::Markdown).searchable(),
                Field::new("status", FieldType::Enum(status::NAMES))
                    .indexed()
                    .default_to("open"),
                Field::new("start", FieldType::Date).indexed(),
                Field::new("due", FieldType::Date).indexed(),
            ],
        },
    ],
};
