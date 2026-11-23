//! Filters generated from a schema: which fields a person can filter by, what they chose, and
//! whether an item passes. The fields are the kind's [`indexed`](Field::indexed) ones that are not
//! [`internal`](Field::internal); a control for each is the frontend's job, by [`FieldType`].
//!
//! The engine's own [`Query`](crate::Query) filters can only AND single comparisons, and a person
//! picking two statuses means "either", so these are applied to the items a query returns.

use std::collections::BTreeMap;

use serde_json::Value;

use super::item::Item;
use super::schema::{Field, FieldType, Kind};

/// What was chosen for one field.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterValue {
    /// The field equals any of these (a status, a priority, an epic, a flag).
    AnyOf(Vec<Value>),
    /// The field lies between these, inclusive; either end may be open (dates, ratings, numbers).
    Range {
        from: Option<Value>,
        to: Option<Value>,
    },
}

/// The fields of `kind` a person can filter by, in schema order.
pub fn filterable(kind: &Kind) -> Vec<&Field> {
    kind.fields
        .iter()
        .filter(|f| f.indexed && !f.internal)
        .filter(|f| {
            matches!(
                f.ty,
                FieldType::Enum(_)
                    | FieldType::Bool
                    | FieldType::Date
                    | FieldType::Number
                    | FieldType::Rating
                    | FieldType::Reference(_)
            )
        })
        .collect()
}

/// Every choice made, by field name. Empty means no filtering.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Filters(BTreeMap<String, FilterValue>);

/// `a` and `b` are the same value; numbers are equal however they were written (`3` and `3.0`).
fn same(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

/// `v` compared with `bound`: numbers by value, dates (ISO text) by text. `None` if they are not
/// comparable.
fn order(v: &Value, bound: &Value) -> Option<std::cmp::Ordering> {
    match (v.as_f64(), bound.as_f64()) {
        (Some(x), Some(y)) => x.partial_cmp(&y),
        _ => Some(v.as_str()?.cmp(bound.as_str()?)),
    }
}

impl Filters {
    /// How many fields are being filtered.
    pub fn count(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get(&self, field: &str) -> Option<&FilterValue> {
        self.0.get(field)
    }

    /// The values chosen for `field` as "any of", or none.
    pub fn any_of(&self, field: &str) -> &[Value] {
        match self.0.get(field) {
            Some(FilterValue::AnyOf(v)) => v,
            _ => &[],
        }
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Add `value` to the choices for `field`, or take it out if it is already chosen. A field
    /// with nothing chosen is not filtered.
    pub fn toggle(&mut self, field: &str, value: Value) {
        let mut chosen = self.any_of(field).to_vec();
        match chosen.iter().position(|v| same(v, &value)) {
            Some(i) => {
                chosen.remove(i);
            }
            None => chosen.push(value),
        }
        if chosen.is_empty() {
            self.0.remove(field);
        } else {
            self.0.insert(field.into(), FilterValue::AnyOf(chosen));
        }
    }

    /// Filter `field` to a range; both ends open removes the filter.
    pub fn set_range(&mut self, field: &str, from: Option<Value>, to: Option<Value>) {
        if from.is_none() && to.is_none() {
            self.0.remove(field);
        } else {
            self.0.insert(field.into(), FilterValue::Range { from, to });
        }
    }

    pub fn remove(&mut self, field: &str) {
        self.0.remove(field);
    }

    /// Whether the item passes every filter. A field the item does not have fails an "any of"
    /// (unless it asks for `false`, which an absent flag is) and any range.
    pub fn matches(&self, item: &Item) -> bool {
        self.0.iter().all(|(name, f)| {
            let v = item.fields.get(name);
            match f {
                FilterValue::AnyOf(any) => match v {
                    Some(v) => any.iter().any(|a| same(a, v)),
                    None => any.contains(&Value::Bool(false)),
                },
                FilterValue::Range { from, to } => v.is_some_and(|v| {
                    from.as_ref()
                        .is_none_or(|b| order(v, b).is_some_and(|o| o.is_ge()))
                        && to
                            .as_ref()
                            .is_none_or(|b| order(v, b).is_some_and(|o| o.is_le()))
                }),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    static KIND: Kind = Kind {
        name: "t",
        fields: &[
            Field::new("notes", FieldType::Markdown).searchable(),
            Field::new("status", FieldType::Enum(&["a", "b"])).indexed(),
            Field::new("pinned", FieldType::Bool).indexed(),
            Field::new("due", FieldType::Date).indexed(),
            Field::new("stars", FieldType::Rating).indexed(),
            Field::new("trashed", FieldType::Number)
                .indexed()
                .internal(),
            Field::new("link", FieldType::Reference("x")).indexed(),
            Field::new("url", FieldType::Url).indexed(),
        ],
    };

    fn item(fields: Value) -> Item {
        Item {
            id: 1,
            kind: "t".into(),
            parent_id: None,
            title: "t".into(),
            fields: fields.as_object().cloned().unwrap_or_default(),
            position: 0,
            archived: false,
            created: 0,
            updated: 0,
            tags: vec![],
        }
    }

    #[test]
    fn only_indexed_non_internal_fields_of_a_filterable_type_are_offered() {
        let names: Vec<_> = filterable(&KIND).iter().map(|f| f.name).collect();
        assert_eq!(names, ["status", "pinned", "due", "stars", "link"]);
    }

    #[test]
    fn picking_two_values_means_either() {
        let mut f = Filters::default();
        f.toggle("status", json!("a"));
        f.toggle("status", json!("b"));
        assert!(f.matches(&item(json!({"status": "a"}))));
        assert!(f.matches(&item(json!({"status": "b"}))));
        f.toggle("status", json!("b"));
        assert!(!f.matches(&item(json!({"status": "b"}))), "unpicked again");
        assert!(!f.matches(&item(json!({}))), "no status at all");
    }

    #[test]
    fn toggling_the_last_choice_off_removes_the_filter() {
        let mut f = Filters::default();
        f.toggle("status", json!("a"));
        assert_eq!(f.count(), 1);
        f.toggle("status", json!("a"));
        assert!(f.is_empty());
        assert!(f.matches(&item(json!({}))));
    }

    #[test]
    fn an_absent_flag_is_false() {
        let mut on = Filters::default();
        on.toggle("pinned", json!(true));
        let mut off = Filters::default();
        off.toggle("pinned", json!(false));
        assert!(on.matches(&item(json!({"pinned": true}))));
        assert!(!on.matches(&item(json!({}))));
        assert!(off.matches(&item(json!({}))));
        assert!(!off.matches(&item(json!({"pinned": true}))));
    }

    #[test]
    fn ranges_compare_numbers_by_value_and_dates_as_text() {
        let mut f = Filters::default();
        f.set_range("stars", Some(json!(4)), None);
        assert!(f.matches(&item(json!({"stars": 5}))));
        assert!(f.matches(&item(json!({"stars": 4.0}))));
        assert!(!f.matches(&item(json!({"stars": 3}))));
        assert!(!f.matches(&item(json!({}))), "unrated fails a minimum");
        let mut d = Filters::default();
        d.set_range("due", Some(json!("2026-11-01")), Some(json!("2026-11-30")));
        assert!(d.matches(&item(json!({"due": "2026-11-15"}))));
        assert!(d.matches(&item(json!({"due": "2026-11-30"}))), "inclusive");
        assert!(!d.matches(&item(json!({"due": "2026-12-01"}))));
        d.set_range("due", None, Some(json!("2026-11-30")));
        assert!(d.matches(&item(json!({"due": "2020-01-01"}))), "open start");
        d.set_range("due", None, None);
        assert!(d.is_empty());
    }

    #[test]
    fn references_match_by_id_and_every_filter_must_pass() {
        let mut f = Filters::default();
        f.toggle("link", json!(7));
        f.toggle("status", json!("a"));
        assert!(f.matches(&item(json!({"link": 7, "status": "a"}))));
        assert!(!f.matches(&item(json!({"link": 8, "status": "a"}))));
        assert!(!f.matches(&item(json!({"link": 7, "status": "b"}))));
    }
}
