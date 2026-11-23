//! # organiser-engine
//!
//! The one headless engine behind every organiser app. An app supplies a [`Schema`] (its item
//! kinds and typed fields); the engine stores items, their hierarchy, tags and a search index in
//! that app's SQLite file. No UI or CLI code lives here.
//!
//! Layers, each importing only the ones above it in this list (`tests/layers.rs` enforces it):
//!
//! * [`primitives`]: pure rules and plain data (schema validation, items, queries, SQL planning).
//! * [`components`]: one SQLite concern each (connection, migrations, rows, tree, tags, search).
//! * [`db`]: the [`Db`] facade that frontends use.

pub mod components;
pub mod db;
pub mod error;
pub mod primitives;

pub use db::Db;
pub use error::{Error, Result};
pub use primitives::filters::{FilterValue, Filters, filterable};
pub use primitives::item::{Item, ItemPatch, NewItem};
pub use primitives::palette::{TAG_PALETTE, pick as pick_tag_color};
pub use primitives::query::{FieldFilter, Op, ParentFilter, Query, Sort};
pub use primitives::schema::{Field, FieldType, Gate, GateVia, Kind, Schema};
pub use primitives::tree::{Node, Row, build as build_tree, flatten as flatten_tree};
pub use weaver::{ChildPolicy, Tag, TagFilter};
