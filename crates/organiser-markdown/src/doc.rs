//! A note's body as a document: the shapes Markdown can express, as plain data. [`parse`]
//! builds one from Markdown and [`export`] writes one back out, so a note round-trips.
//!
//! [`parse`]: crate::parse
//! [`export`]: crate::export

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Inline {
    Text(String),
    Code(String),
    Emphasis(Vec<Inline>),
    Strong(Vec<Inline>),
    Strike(Vec<Inline>),
    Highlight(Vec<Inline>),
    Subscript(Vec<Inline>),
    Superscript(Vec<Inline>),
    Link { content: Vec<Inline>, url: String },
    Image { alt: String, url: String },
    FootnoteRef(String),
    LineBreak,
}

/// A list entry; `checked` is `Some` for a task-list item (`- [ ]` / `- [x]`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Item {
    pub checked: Option<bool>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Definition {
    pub term: Vec<Inline>,
    pub details: Vec<Vec<Inline>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Footnote {
    pub label: String,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Block {
    Heading {
        level: u8,
        id: Option<String>,
        content: Vec<Inline>,
    },
    Paragraph(Vec<Inline>),
    Quote(Vec<Block>),
    List {
        start: Option<u64>,
        items: Vec<Item>,
    },
    CodeBlock {
        lang: Option<String>,
        code: String,
    },
    Rule,
    Table {
        align: Vec<Align>,
        head: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    Definitions(Vec<Definition>),
    /// Footnote definitions, always last in a document.
    Footnotes(Vec<Footnote>),
}
