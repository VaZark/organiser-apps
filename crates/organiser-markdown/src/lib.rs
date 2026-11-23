//! # organiser-markdown
//!
//! Markdown as the organiser apps use it, with no GUI and no database: [`parse`] reads Markdown
//! into a [`doc`] (plain data), [`export`] writes one back (so `parse(export(doc)) == doc`), and
//! [`tasks`] ticks a task in the source text. The Markdown Guide's cheat sheet, basic and
//! extended, is covered. How a document is drawn is the toolkit's business.

pub mod doc;
pub mod export;
pub mod parse;
pub mod tasks;

pub use doc::{Align, Block, Definition, Footnote, Inline, Item};
