//! Gated fields: a field that exists only while a flag is on in the item it is gated through.

use rusqlite::{Connection, params};
use serde_json::{Map, Value};

use crate::error::{Result, invalid};
use crate::primitives::schema::{GateVia, Schema};

fn flag_is_on(conn: &Connection, flag: &str, holder: i64) -> bool {
    let on: Option<i64> = conn
        .query_row(
            &format!("SELECT json_extract(fields, '$.{flag}') FROM items WHERE id = ?1"),
            [holder],
            |r| r.get(0),
        )
        .unwrap_or(None);
    on == Some(1)
}

/// Every gated field present in `fields` must have its gate open. `parent` is the item's parent.
pub fn check(
    conn: &Connection,
    schema: &Schema,
    kind: &str,
    parent: Option<i64>,
    fields: &Map<String, Value>,
) -> Result<()> {
    for (field, gate) in schema.gates(kind) {
        if !fields.contains_key(field) {
            continue;
        }
        let holder = match gate.via {
            GateVia::Field(f) => fields.get(f).and_then(Value::as_i64),
            GateVia::Parent(k) => parent.filter(|p| {
                conn.query_row("SELECT kind FROM items WHERE id = ?1", [p], |r| {
                    r.get::<_, String>(0)
                })
                .is_ok_and(|pk| pk == k)
            }),
        };
        let Some(holder) = holder else {
            return invalid(format!(
                "{field} needs a {}",
                match gate.via {
                    GateVia::Field(f) => f.to_string(),
                    GateVia::Parent(k) => format!("parent {k}"),
                }
            ));
        };
        if !flag_is_on(conn, gate.flag, holder) {
            return invalid(format!("{field} is not switched on ({} is off)", gate.flag));
        }
    }
    Ok(())
}

/// After an item of `kind` was written with `fields`: every gated field whose flag is now off is
/// removed from the items that depend on it. Switching a flag off removes what it allowed.
pub fn strip(
    conn: &Connection,
    schema: &Schema,
    kind: &str,
    id: i64,
    fields: &Map<String, Value>,
) -> Result<()> {
    for (k, field, gate) in schema.gated_by(kind) {
        if fields.get(gate.flag).and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let who = match gate.via {
            GateVia::Field(via) => format!("json_extract(fields, '$.{via}')"),
            GateVia::Parent(_) => "parent_id".to_string(),
        };
        conn.execute(
            &format!(
                "UPDATE items SET fields = json_remove(fields, '$.{field}')
                 WHERE kind = ?1 AND {who} = ?2 AND json_extract(fields, '$.{field}') IS NOT NULL"
            ),
            params![k, id],
        )?;
    }
    Ok(())
}
