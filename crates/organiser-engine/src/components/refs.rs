//! Links between items: checking a link's target, and removing links to a deleted item.

use rusqlite::{Connection, params};

use crate::error::{Error, Result, invalid};
use crate::primitives::schema::Schema;

/// Each `(id, kind)` must be an existing item of that kind.
pub fn check(conn: &Connection, targets: &[(i64, &'static str)]) -> Result<()> {
    for (id, kind) in targets {
        let found: Option<String> = conn
            .query_row("SELECT kind FROM items WHERE id = ?1", [id], |r| r.get(0))
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                e => Err(e),
            })?;
        match found {
            None => return Err(Error::NotFound(*id)),
            Some(k) if k != *kind => return invalid(format!("item {id} is a {k}, not a {kind}")),
            Some(_) => {}
        }
    }
    Ok(())
}

/// Remove every reference to the deleted items (given with their kinds) from other items.
pub fn unlink(conn: &Connection, schema: &Schema, deleted: &[(i64, String)]) -> Result<()> {
    for (id, kind) in deleted {
        for (k, field) in schema.referrers(kind) {
            conn.execute(
                &format!(
                    "UPDATE items SET fields = json_remove(fields, '$.{field}')
                     WHERE kind = ?1 AND json_extract(fields, '$.{field}') = ?2"
                ),
                params![k, id],
            )?;
        }
    }
    Ok(())
}

/// Used by writes: the fields' links must all resolve.
pub fn check_fields(
    conn: &Connection,
    schema: &Schema,
    kind: &str,
    fields: &serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    check(conn, &schema.references(kind, fields))?;
    Ok(())
}
