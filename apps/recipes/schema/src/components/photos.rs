//! The photos folder: copying a picked picture in, finding it again, and clearing what nothing
//! uses any more. The recipes keep only the names (see [`crate::photo`]).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use organiser_core::paths;
use organiser_engine::{Db, Error, Query, Result};
use serde_json::Value;

use crate::primitives::photo;
use crate::{META, RECIPE, STEP};

/// The biggest picture accepted: a photo is read whole, and the file picker may offer anything.
pub const MAX_BYTES: u64 = 40 * 1024 * 1024;

/// Where this app's photos live.
pub fn dir() -> PathBuf {
    paths::data_dir(&META).join("photos")
}

/// Copy `source` into `dir` and return its name. The same picture added again is the same name
/// and is not stored twice.
pub fn import(dir: &Path, source: &Path) -> Result<String> {
    let ext = photo::extension(source).ok_or_else(|| {
        Error::Invalid(format!(
            "{} is not a picture this app shows ({})",
            source.display(),
            photo::EXTENSIONS.join(", ")
        ))
    })?;
    let size = std::fs::metadata(source)
        .map_err(|e| Error::Invalid(format!("cannot read {}: {e}", source.display())))?
        .len();
    if size > MAX_BYTES {
        return Err(Error::Invalid(format!(
            "{} is {} MB; the most is {} MB",
            source.display(),
            size / 1_048_576,
            MAX_BYTES / 1_048_576
        )));
    }
    let bytes = std::fs::read(source)
        .map_err(|e| Error::Invalid(format!("cannot read {}: {e}", source.display())))?;
    let name = photo::name_for(&bytes, ext);
    let target = dir.join(&name);
    if !target.exists() {
        std::fs::create_dir_all(dir)
            .map_err(|e| Error::Invalid(format!("cannot create {}: {e}", dir.display())))?;
        std::fs::write(&target, bytes)
            .map_err(|e| Error::Invalid(format!("cannot write {}: {e}", target.display())))?;
    }
    Ok(name)
}

/// The file for a photo name, or `None` for a name this app did not make.
pub fn path(dir: &Path, name: &str) -> Option<PathBuf> {
    photo::is_valid_name(name).then(|| dir.join(name))
}

/// Delete the photos in `dir` that no recipe or step lists. Returns how many files went.
pub fn gc(db: &Db, dir: &Path) -> Result<usize> {
    let mut used: HashSet<String> = HashSet::new();
    for kind in [RECIPE, STEP] {
        for item in db.list(&Query {
            kind: Some(kind.into()),
            ..Query::default()
        })? {
            if let Some(Value::Array(a)) = item.fields.get("photos") {
                used.extend(a.iter().filter_map(Value::as_str).map(String::from));
            }
        }
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(0);
    };
    let mut removed = 0;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        // Only files this app made, and only when nothing lists them.
        if photo::is_valid_name(&name)
            && !used.contains(&name)
            && std::fs::remove_file(e.path()).is_ok()
        {
            removed += 1;
        }
    }
    Ok(removed)
}
