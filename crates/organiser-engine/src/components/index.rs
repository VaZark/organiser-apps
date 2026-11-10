//! Expression indexes for the fields a schema marks `indexed`. Derived from the schema on every
//! open, so changing a schema never needs a migration for these.

use rusqlite::Connection;

use crate::error::Result;
use crate::primitives::schema::Schema;

pub fn reconcile(conn: &Connection, schema: &Schema) -> Result<()> {
    for f in schema
        .kinds
        .iter()
        .flat_map(|k| k.fields)
        .filter(|f| f.indexed)
    {
        conn.execute_batch(&format!(
            "CREATE INDEX IF NOT EXISTS idx_fld_{n} ON items(json_extract(fields,'$.{n}'))",
            n = f.name
        ))?;
    }
    Ok(())
}
