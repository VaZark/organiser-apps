//! The engine's own schema, versioned with `PRAGMA user_version`. Migrations only ever append.

use rusqlite::Connection;

use super::tags::TAGS;
use crate::error::{Error, Result};

const V1: &str = "
CREATE TABLE items (
    id        INTEGER PRIMARY KEY,
    kind      TEXT NOT NULL,
    parent_id INTEGER REFERENCES items(id) ON DELETE CASCADE,
    title     TEXT NOT NULL,
    fields    TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(fields)),
    position  INTEGER NOT NULL DEFAULT 0,
    archived  INTEGER NOT NULL DEFAULT 0,
    created   INTEGER NOT NULL,
    updated   INTEGER NOT NULL
);
CREATE INDEX items_parent ON items(parent_id, position);
CREATE INDEX items_kind ON items(kind, archived);
CREATE VIRTUAL TABLE items_fts USING fts5(title, body, tokenize = 'unicode61 remove_diacritics 2');
";

fn migrations() -> Vec<String> {
    vec![format!("{V1}\n{}", TAGS.schema_sql())]
}

pub fn run(conn: &mut Connection) -> Result<()> {
    let all = migrations();
    let current: usize = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if current > all.len() {
        return Err(Error::Schema(format!(
            "database is version {current}, this build only knows {}",
            all.len()
        )));
    }
    for (i, sql) in all.iter().enumerate().skip(current) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", i + 1)?;
        tx.commit()?;
    }
    Ok(())
}
