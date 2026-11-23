//! Tags on items, through weaver. Items are the one tag owner; the vocabulary and the tag tree
//! are shared by every kind of item in the database.

use rusqlite::{Connection, Transaction};
use weaver::{Owner, TagName, Tags};

use crate::error::Result;

pub const OWNER: Owner = Owner {
    name: "item",
    table: "item_tags",
    col: "item_id",
    owner_table: "items",
    owner_key: "id",
    key_type: "INTEGER",
};
static OWNERS: [Owner; 1] = [OWNER];
pub const TAGS: Tags = Tags::new(&OWNERS);

/// Parse every name first so a bad one fails before anything is written.
pub fn parse_all(names: &[String]) -> Result<Vec<TagName>> {
    Ok(names
        .iter()
        .map(|n| TagName::parse(n))
        .collect::<std::result::Result<_, _>>()?)
}

pub fn assign(tx: &Transaction, id: i64, names: &[TagName]) -> Result<usize> {
    let mut added = 0;
    for n in names {
        added += usize::from(TAGS.assign(tx, &OWNER, &id.to_string(), n)?);
    }
    Ok(added)
}

pub fn remove(conn: &mut Connection, id: i64, names: &[String]) -> Result<usize> {
    Ok(TAGS.remove(conn, &OWNER, &id.to_string(), names)?)
}

pub fn of(conn: &Connection, id: i64) -> Result<Vec<String>> {
    Ok(TAGS.of(conn, &OWNER, &id.to_string())?)
}
