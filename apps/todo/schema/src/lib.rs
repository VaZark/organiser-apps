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

pub use components::{lifecycle, statuses, views};
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
pub const STATUS: &str = "status";

pub static SCHEMA: Schema = Schema {
    kinds: &[
        Kind {
            name: TASK,
            fields: &[
                Field::new("notes", FieldType::Markdown).searchable(),
                // The id of one of the person's statuses (see `statuses`).
                Field::new("status", FieldType::Reference(STATUS)).indexed(),
                Field::new("priority", FieldType::Enum(priority::NAMES))
                    .indexed()
                    .default_to("medium"),
                Field::new("start", FieldType::Date).indexed(),
                Field::new("due", FieldType::Date).indexed(),
                // Filtered through the epics themselves (their own section and sidebar), so not
                // offered a second time among the generated filters.
                Field::new("epic", FieldType::Reference(EPIC))
                    .indexed()
                    .internal(),
            ],
        },
        Kind {
            name: STATUS,
            fields: &[
                // `#rrggbb`.
                Field::new("color", FieldType::Text),
                // Finished: done, or decided against.
                Field::new("closed", FieldType::Bool).indexed(),
            ],
        },
        Kind {
            name: EPIC,
            fields: &[
                Field::new("notes", FieldType::Markdown).searchable(),
                // The id of one of the person's statuses (see `statuses`).
                Field::new("status", FieldType::Reference(STATUS)).indexed(),
                Field::new("start", FieldType::Date).indexed(),
                Field::new("due", FieldType::Date).indexed(),
            ],
        },
    ],
};
