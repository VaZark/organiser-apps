//! The full-text index, kept in step by the engine on every write.

use rusqlite::{Connection, params};

use crate::error::Result;

pub fn index(conn: &Connection, id: i64, title: &str, body: &str) -> Result<()> {
    conn.execute("DELETE FROM items_fts WHERE rowid = ?1", [id])?;
    conn.execute(
        "INSERT INTO items_fts(rowid, title, body) VALUES (?1, ?2, ?3)",
        params![id, title, body],
    )?;
    Ok(())
}

pub fn remove(conn: &Connection, ids: &[i64]) -> Result<()> {
    for id in ids {
        conn.execute("DELETE FROM items_fts WHERE rowid = ?1", [id])?;
    }
    Ok(())
}
