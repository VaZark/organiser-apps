//! The data types every frontend exchanges with the engine.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: i64,
    pub kind: String,
    pub parent_id: Option<i64>,
    pub title: String,
    pub fields: Map<String, Value>,
    /// Order among siblings (ascending).
    pub position: i64,
    pub archived: bool,
    /// Unix seconds.
    pub created: i64,
    pub updated: i64,
    /// Tags in display spelling (`key:Value` or `Value`).
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NewItem {
    pub kind: String,
    pub parent_id: Option<i64>,
    pub title: String,
    #[serde(default)]
    pub fields: Map<String, Value>,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl NewItem {
    pub fn new(kind: &str, title: &str) -> Self {
        Self {
            kind: kind.into(),
            title: title.into(),
            ..Self::default()
        }
    }
    pub fn parent(mut self, id: i64) -> Self {
        self.parent_id = Some(id);
        self
    }
    pub fn field(mut self, name: &str, v: impl Into<Value>) -> Self {
        self.fields.insert(name.into(), v.into());
        self
    }
    pub fn tag(mut self, t: &str) -> Self {
        self.tags.push(t.into());
        self
    }
}

/// A partial update. `fields` is merged into the existing fields; a `null` value removes one.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ItemPatch {
    pub title: Option<String>,
    #[serde(default)]
    pub fields: Map<String, Value>,
    pub archived: Option<bool>,
}

impl ItemPatch {
    pub fn title(mut self, t: &str) -> Self {
        self.title = Some(t.into());
        self
    }
    pub fn field(mut self, name: &str, v: impl Into<Value>) -> Self {
        self.fields.insert(name.into(), v.into());
        self
    }
    pub fn archived(mut self, a: bool) -> Self {
        self.archived = Some(a);
        self
    }
}
