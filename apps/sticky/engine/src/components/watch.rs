//! Noticing that something Sticky shows may have changed, without reading anything: only the
//! modification time and size of a few files are looked at. No polling of databases, no parsing.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use organiser_sticky::store;

/// What a path looked like: nothing, or its size and modification time.
fn stamp(p: &Path) -> Option<(u64, SystemTime)> {
    let m = std::fs::metadata(p).ok()?;
    Some((m.len(), m.modified().ok()?))
}

/// A hash of the stamps of every file whose change could change what Sticky shows: the registry,
/// the registrations in the drop-in folder, and the linked databases with their write-ahead logs
/// (where an app's write lands first).
fn fingerprint(extra_dbs: &[PathBuf]) -> u64 {
    let mut h = DefaultHasher::new();
    let links = store::path();
    stamp(&links).hash(&mut h);
    stamp(&store::local_path()).hash(&mut h);
    let mut files: Vec<PathBuf> = std::fs::read_dir(store::dropin_dir())
        .map(|d| d.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    files.sort();
    for f in &files {
        f.hash(&mut h);
        stamp(f).hash(&mut h);
    }
    for db in extra_dbs {
        stamp(db).hash(&mut h);
        let mut wal = db.as_os_str().to_owned();
        wal.push("-wal");
        stamp(Path::new(&wal)).hash(&mut h);
    }
    h.finish()
}

/// Remembers the last fingerprint; [`Watcher::changed`] says whether it is different now.
#[derive(Default)]
pub struct Watcher {
    last: Option<u64>,
    registry: Option<u64>,
    dbs: Vec<PathBuf>,
}

impl Watcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether anything watched changed since the last call (the first call says yes). When the
    /// registry itself changed, the list of databases to watch is read again.
    pub fn changed(&mut self) -> bool {
        let registry = fingerprint(&[]);
        if self.registry != Some(registry) {
            self.registry = Some(registry);
            self.dbs = store::load()
                .map(|l| l.enabled().map(|l| PathBuf::from(&l.db_path)).collect())
                .unwrap_or_default();
        }
        let now = fingerprint(&self.dbs);
        let changed = self.last != Some(now);
        self.last = Some(now);
        changed
    }
}
