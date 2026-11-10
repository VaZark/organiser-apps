//! The parent/child hierarchy and sibling order.

use rusqlite::{Connection, params};

use crate::error::{Result, invalid};
use crate::primitives::clock::now;

/// The item and everything below it.
pub fn subtree(conn: &Connection, id: i64) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "WITH RECURSIVE d(id) AS (SELECT ?1 UNION SELECT i.id FROM items i JOIN d ON i.parent_id = d.id)
         SELECT id FROM d",
    )?;
    let rows = stmt.query_map([id], |r| r.get(0))?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// The position after the last child of `parent` (`None` = top level).
pub fn next_position(conn: &Connection, parent: Option<i64>) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM items WHERE parent_id IS ?1",
        [parent],
        |r| r.get(0),
    )?)
}

/// Re-parent and/or reorder. `position: None` appends. Refuses to put an item under itself.
pub fn relocate(
    conn: &Connection,
    id: i64,
    parent: Option<i64>,
    position: Option<i64>,
) -> Result<()> {
    if let Some(p) = parent
        && subtree(conn, id)?.contains(&p)
    {
        return invalid("an item cannot move under itself or its own descendants");
    }
    let pos = match position {
        Some(p) => {
            conn.execute(
                "UPDATE items SET position = position + 1
                 WHERE parent_id IS ?1 AND position >= ?2 AND id <> ?3",
                params![parent, p, id],
            )?;
            p
        }
        None => next_position(conn, parent)?,
    };
    conn.execute(
        "UPDATE items SET parent_id = ?1, position = ?2, updated = ?3 WHERE id = ?4",
        params![parent, pos, now(), id],
    )?;
    Ok(())
}
