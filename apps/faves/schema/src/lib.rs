//! # faves-schema
//!
//! The Faves app: several **lists** (movies, cars, animals, anything), each holding **entries** you
//! track with optional notes or an opinion. Which extras a list offers is the list's own choice:
//! `rated` adds a star rating to its entries. An entry belongs to its list as a child item, so
//! deleting a list deletes its entries, and the engine only lets an entry carry a rating while its
//! list has ratings switched on.
//!
//! Layers, as in the engine (`tests/layers.rs` enforces them): [`primitives`] (the pure model) and
//! [`components`] (operations on a [`organiser_engine::Db`]).

pub mod components;
pub mod primitives;

use organiser_core::AppMeta;
use organiser_engine::{Field, FieldType, Kind, Schema};

pub use components::{lifecycle, views};
pub use primitives::model;

pub const META: AppMeta = AppMeta {
    id: "io.github.vazark.Faves",
    slug: "faves",
    name: "Faves",
    comment: "Track the movies, cars, animals and anything else you like",
    categories: &["Utility"],
};

pub const LIST: &str = "list";
pub const ENTRY: &str = "entry";

pub static SCHEMA: Schema = Schema {
    kinds: &[
        Kind {
            name: LIST,
            fields: &[
                Field::new("notes", FieldType::Markdown).searchable(),
                // Entries of this list can be rated.
                Field::new("rated", FieldType::Bool),
            ],
        },
        Kind {
            name: ENTRY,
            fields: &[
                Field::new("notes", FieldType::Markdown).searchable(),
                Field::new("favorite", FieldType::Bool).indexed(),
                Field::new("rating", FieldType::Rating)
                    .indexed()
                    .gated_by_parent(LIST, "rated"),
            ],
        },
    ],
};
