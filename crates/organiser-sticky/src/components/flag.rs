//! The sticky flag on items of a [`Db`]. The kind's schema must have the `sticky` field.

use organiser_engine::{Db, FieldFilter, Item, ItemPatch, Op, Query, Result};
use serde_json::Value;

use crate::{STICKY, STICKY_COLOR};

/// Make an item a sticky (or not). Unflagging drops its colour too.
pub fn set_sticky(db: &mut Db, id: i64, on: bool) -> Result<Item> {
    let patch = if on {
        ItemPatch::default().field(STICKY, true)
    } else {
        ItemPatch::default()
            .field(STICKY, Value::Null)
            .field(STICKY_COLOR, Value::Null)
    };
    db.update(id, patch)
}

/// Set a sticky's colour (`#rrggbb`); `None` is the default yellow.
pub fn set_color(db: &mut Db, id: i64, color: Option<&str>) -> Result<Item> {
    let v = color.map_or(Value::Null, |c| Value::String(c.into()));
    db.update(id, ItemPatch::default().field(STICKY_COLOR, v))
}

pub fn is_sticky(item: &Item) -> bool {
    item.fields.get(STICKY).and_then(Value::as_bool) == Some(true)
}

pub fn color(item: &Item) -> Option<&str> {
    item.fields.get(STICKY_COLOR).and_then(Value::as_str)
}

/// Every flagged item of `kind` that is not archived.
pub fn flagged(db: &Db, kind: &str) -> Result<Vec<Item>> {
    db.list(&Query {
        kind: Some(kind.into()),
        archived: Some(false),
        filters: vec![FieldFilter::new(STICKY, Op::Eq, true)],
        ..Query::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use organiser_engine::NewItem;

    #[test]
    fn flagging_lists_the_item_and_unflagging_drops_it_and_its_colour() {
        let mut db = Db::open_in_memory(&notes_schema::SCHEMA).unwrap();
        let a = db.create(NewItem::new(notes_schema::NOTE, "a")).unwrap();
        db.create(NewItem::new(notes_schema::NOTE, "b")).unwrap();
        assert!(flagged(&db, notes_schema::NOTE).unwrap().is_empty());

        set_sticky(&mut db, a.id, true).unwrap();
        let it = set_color(&mut db, a.id, Some("#ffd54f")).unwrap();
        assert!(is_sticky(&it));
        assert_eq!(color(&it), Some("#ffd54f"));
        let f = flagged(&db, notes_schema::NOTE).unwrap();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].id, a.id);

        let it = set_sticky(&mut db, a.id, false).unwrap();
        assert!(!is_sticky(&it) && color(&it).is_none());
        assert!(flagged(&db, notes_schema::NOTE).unwrap().is_empty());
    }
}
