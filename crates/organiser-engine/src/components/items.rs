//! Item rows: reading them into [`Item`]s and writing them back.

use rusqlite::{Connection, Row, params};
use serde_json::{Map, Value};

use super::tags;
use crate::error::{Error, Result};
use crate::primitives::item::Item;

pub const COLUMNS: &str = "items.id, items.kind, items.parent_id, items.title, items.fields, items.position, items.archived, items.created, items.updated";

pub fn from_row(r: &Row) -> rusqlite::Result<Item> {
    let fields: String = r.get(4)?;
    Ok(Item {
        id: r.get(0)?,
        kind: r.get(1)?,
        parent_id: r.get(2)?,
        title: r.get(3)?,
        fields: serde_json::from_str::<Map<String, Value>>(&fields).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
        })?,
        position: r.get(5)?,
        archived: r.get::<_, i64>(6)? != 0,
        created: r.get(7)?,
        updated: r.get(8)?,
        tags: Vec::new(),
    })
}

/// Attach each item's tags.
pub fn with_tags(conn: &Connection, mut items: Vec<Item>) -> Result<Vec<Item>> {
    for i in &mut items {
        i.tags = tags::of(conn, i.id)?;
    }
    Ok(items)
}

pub fn get(conn: &Connection, id: i64) -> Result<Item> {
    let item = conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM items WHERE id = ?1"),
            [id],
            from_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Error::NotFound(id),
            e => e.into(),
        })?;
    Ok(with_tags(conn, vec![item])?.remove(0))
}

pub fn exists(conn: &Connection, id: i64) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM items WHERE id = ?1)",
        [id],
        |r| r.get(0),
    )?)
}

pub fn insert(
    conn: &Connection,
    kind: &str,
    parent: Option<i64>,
    title: &str,
    fields: &Map<String, Value>,
    position: i64,
    now: i64,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO items(kind, parent_id, title, fields, position, created, updated)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![
            kind,
            parent,
            title,
            serde_json::to_string(fields)?,
            position,
            now
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn write(
    conn: &Connection,
    id: i64,
    title: &str,
    fields: &Map<String, Value>,
    archived: bool,
    now: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE items SET title = ?1, fields = ?2, archived = ?3, updated = ?4 WHERE id = ?5",
        params![
            title,
            serde_json::to_string(fields)?,
            i64::from(archived),
            now,
            id
        ],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM items WHERE id = ?1", [id])?;
    Ok(())
}
