//! What a command produces, and how it prints as text or JSON.

use organiser_engine::{Item, Tag};
use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub enum Output {
    Item(Item),
    Items(Vec<Item>),
    Tags(Vec<Tag>),
    Message(String),
    /// An app's own result: the text to print and the JSON for `--json`.
    Custom {
        human: String,
        json: Value,
    },
}

pub fn item_line(i: &Item) -> String {
    let mut s = format!("{:>4}  {}", i.id, i.title);
    if !i.tags.is_empty() {
        s.push_str(&format!("  [{}]", i.tags.join(", ")));
    }
    for (k, v) in &i.fields {
        let v = v.as_str().map_or_else(|| v.to_string(), str::to_string);
        if !v.contains('\n') && v.len() <= 40 {
            s.push_str(&format!("  {k}={v}"));
        }
    }
    s
}

fn item_detail(i: &Item) -> String {
    let mut s = format!("#{} {}  ({})\n", i.id, i.title, i.kind);
    if let Some(p) = i.parent_id {
        s.push_str(&format!("parent: {p}\n"));
    }
    if !i.tags.is_empty() {
        s.push_str(&format!("tags: {}\n", i.tags.join(", ")));
    }
    if i.archived {
        s.push_str("archived\n");
    }
    for (k, v) in &i.fields {
        let v = v.as_str().map_or_else(|| v.to_string(), str::to_string);
        s.push_str(&format!("{k}: {v}\n"));
    }
    s.trim_end().to_string()
}

impl Output {
    pub fn render(&self, as_json: bool) -> String {
        if as_json {
            let v = match self {
                Output::Item(i) => json!(i),
                Output::Items(v) => json!(v),
                Output::Tags(v) => json!(v),
                Output::Message(m) => json!({ "message": m }),
                Output::Custom { json, .. } => json.clone(),
            };
            return serde_json::to_string_pretty(&v).unwrap_or_default();
        }
        match self {
            Output::Item(i) => item_detail(i),
            Output::Items(v) if v.is_empty() => "(none)".into(),
            Output::Items(v) => v.iter().map(item_line).collect::<Vec<_>>().join("\n"),
            Output::Tags(v) if v.is_empty() => "(none)".into(),
            Output::Tags(v) => v
                .iter()
                .map(|t| match &t.parent {
                    Some(p) => format!("{:<24} ({}) under {p}", t.name, t.count("item")),
                    None => format!("{:<24} ({})", t.name, t.count("item")),
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Output::Message(m) => m.clone(),
            Output::Custom { human, .. } => human.clone(),
        }
    }
}
