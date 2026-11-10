//! Opening a connection with the pragmas every app wants.

use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

use crate::error::Result;

fn configure(conn: &Connection, wal: bool) -> Result<()> {
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    if wal {
        // Several processes (GUI and CLI) may share the file.
        conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
    }
    Ok(())
}

/// Open (creating the file and its directory if needed).
pub fn open(path: &Path) -> Result<Connection> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let conn = Connection::open(path)?;
    configure(&conn, true)?;
    Ok(conn)
}

pub fn open_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    configure(&conn, false)?;
    Ok(conn)
}
