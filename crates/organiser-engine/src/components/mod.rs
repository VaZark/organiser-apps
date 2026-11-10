//! One SQLite concern each, as free functions over a connection or transaction: connecting,
//! migrating, indexing, item rows, the hierarchy, full-text search, tags and running queries.
//! They import [`crate::primitives`] and each other, never [`crate::db`].

pub mod connection;
pub mod finder;
pub mod fts;
pub mod gates;
pub mod index;
pub mod items;
pub mod migrate;
pub mod refs;
pub mod tags;
pub mod tree;
