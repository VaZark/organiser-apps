//! Headless rules and plain data: the schema and its validation, the item and query types, the
//! query compiler and search-text handling. No SQLite, no I/O.

pub mod clock;
pub mod fts;
pub mod item;
pub mod plan;
pub mod query;
pub mod schema;
pub mod tree;
