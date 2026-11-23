//! What to list: filters, sorting and paging, as plain data.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use weaver::TagFilter;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParentFilter {
    #[default]
    Any,
    Root,
    Of(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    /// Substring match (case-insensitive for ASCII).
    Contains,
    IsNull,
    NotNull,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldFilter {
    /// A schema field or one of `id, parent_id, title, position, archived, created, updated`.
    pub field: String,
    pub op: Op,
    #[serde(default)]
    pub value: Value,
}

impl FieldFilter {
    pub fn new(field: &str, op: Op, value: impl Into<Value>) -> Self {
        Self {
            field: field.into(),
            op,
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sort {
    pub field: String,
    #[serde(default)]
    pub desc: bool,
}

impl Sort {
    pub fn asc(field: &str) -> Self {
        Self {
            field: field.into(),
            desc: false,
        }
    }
    pub fn desc(field: &str) -> Self {
        Self {
            field: field.into(),
            desc: true,
        }
    }
}

/// What to list. Everything is optional; the default lists every item by position.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Query {
    pub kind: Option<String>,
    #[serde(default)]
    pub parent: ParentFilter,
    pub archived: Option<bool>,
    /// Full-text search over the title and the kind's searchable fields.
    pub text: Option<String>,
    #[serde(default)]
    pub tags: TagFilter,
    #[serde(default)]
    pub filters: Vec<FieldFilter>,
    #[serde(default)]
    pub sort: Vec<Sort>,
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: usize,
}
