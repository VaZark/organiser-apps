//! What the apps' vaults do the same way, whatever their layout: settle which file is which item,
//! match files to items, keep what the person deleted out of the app, give items their ids and
//! tell what a deleted item's file was. Each app keeps what is its own: how directories and file
//! names stand for its items, and what is in a file.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::Hash;
use std::path::{Path, PathBuf};

use organiser_engine::{Db, Error, Item, ItemPatch, Query, Result};
use serde_json::Value;

use crate::{Known, KnownFile, uuid7};

/// A text field of an item, if it has one that is not empty.
pub fn field<'a>(item: &'a Item, key: &str) -> Option<&'a str> {
    item.fields
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

// ---- which file is which ----------------------------------------------------------------------

/// A file read from the vault, as far as settling who owns an id goes.
pub trait FoundFile {
    /// The file, relative to the vault.
    fn rel(&self) -> &str;
    /// When the file's item was made, as the file says.
    fn created(&self) -> Option<i64>;
    /// The id the file claims.
    fn uid(&self) -> Option<&str>;
    /// Give up the claim: the file is an item of its own, and gets an id of its own when written.
    fn lose_claim(&mut self);
}

/// Two files claiming one id: the one first by path keeps it, then the older; the others lose
/// the claim.
pub fn settle_claims<F: FoundFile>(found: &mut [F]) {
    let mut claims: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, f) in found.iter().enumerate() {
        if let Some(id) = f.uid() {
            claims.entry(id.to_string()).or_default().push(i);
        }
    }
    for (_, mut idx) in claims {
        if idx.len() < 2 {
            continue;
        }
        idx.sort_by(|a, b| {
            let (fa, fb) = (&found[*a], &found[*b]);
            fa.rel()
                .to_lowercase()
                .cmp(&fb.rel().to_lowercase())
                .then(fa.rel().cmp(fb.rel()))
                .then(
                    fa.created()
                        .unwrap_or(i64::MAX)
                        .cmp(&fb.created().unwrap_or(i64::MAX)),
                )
        });
        for loser in &idx[1..] {
            found[*loser].lose_claim();
        }
    }
}

/// Matches the files found to the items the database has. `K` is what places a plain file that
/// is not tied to an item yet: where it is filed, with its title.
pub struct Matcher<K> {
    by_uid: HashMap<String, i64>,
    by_file: HashMap<String, i64>,
    untied: HashMap<K, i64>,
    seen: HashSet<i64>,
}

impl<K: Hash + Eq> Matcher<K> {
    /// `place` says where an item is filed and under what title, for the items with no file.
    pub fn new(items: &[Item], place: impl Fn(&Item) -> K) -> Self {
        let mut m = Self {
            by_uid: HashMap::new(),
            by_file: HashMap::new(),
            untied: HashMap::new(),
            seen: HashSet::new(),
        };
        for i in items {
            if let Some(u) = field(i, "uid") {
                m.by_uid.insert(u.to_string(), i.id);
            }
            match field(i, "file") {
                Some(f) => {
                    m.by_file.insert(f.to_string(), i.id);
                }
                None => {
                    m.untied.insert(place(i), i.id);
                }
            }
        }
        m
    }

    /// The item a file is: the one with the id it carries; else, for a file with no id (or one
    /// whose claim was lost), the item that has its path, else one filed where the file is with
    /// its title. An id no item has, from another database, names nothing here.
    pub fn find(&mut self, uid: Option<&str>, rel: &str, place: K) -> Option<i64> {
        uid.and_then(|u| self.by_uid.get(u).copied())
            .filter(|id| !self.seen.contains(id))
            .or_else(|| {
                if uid.is_some_and(|u| self.by_uid.contains_key(u)) {
                    return None;
                }
                self.by_file
                    .get(rel)
                    .copied()
                    .filter(|id| !self.seen.contains(id))
                    .or_else(|| self.untied.remove(&place))
            })
    }

    /// Note that an item has its file in this pass.
    pub fn see(&mut self, id: i64) {
        self.seen.insert(id);
    }

    pub fn seen(&self, id: i64) -> bool {
        self.seen.contains(&id)
    }
}

// ---- ids ------------------------------------------------------------------------------------

/// The id the file carries becomes the item's. Returns the item's id after.
pub fn adopt_uid(
    db: &mut Db,
    id: i64,
    current: Option<String>,
    file: Option<&String>,
) -> Result<Option<String>> {
    match file {
        Some(u) if current.as_ref() != Some(u) => {
            db.update(id, ItemPatch::default().field("uid", u.as_str()))?;
            Ok(Some(u.clone()))
        }
        _ => Ok(current),
    }
}

/// An item gets its id the first time it is written.
pub fn ensure_uid(db: &mut Db, id: i64, current: Option<String>) -> Result<String> {
    match current {
        Some(u) => Ok(u),
        None => {
            let u = uuid7();
            db.update(id, ItemPatch::default().field("uid", u.as_str()))?;
            Ok(u)
        }
    }
}

/// Whether the item's file has no id yet; if so, it is marked to be written by the next flush.
/// This is how a file gets its front matter when first opened, not when found.
pub fn mark_unstamped(
    db: &mut Db,
    dir: &Path,
    id: i64,
    uid_of: impl Fn(&str) -> Option<String>,
) -> Result<bool> {
    let item = db.get(id)?;
    let Some(file) = field(&item, "file") else {
        return Ok(false);
    };
    let bare = fs::read_to_string(dir.join(file)).is_ok_and(|t| uid_of(&t).is_none());
    if bare {
        db.update(id, ItemPatch::default().field("file_sig", ""))?;
    }
    Ok(bare)
}

// ---- what the app holds ------------------------------------------------------------------------

/// The file each of `items` has, to hand back to a flush: that is how an item deleted for good is
/// known to have had one.
pub fn known_of(items: &[Item]) -> Known {
    items
        .iter()
        .filter_map(|i| {
            field(i, "file").map(|f| {
                (
                    i.id,
                    KnownFile {
                        rel: f.to_string(),
                        uid: field(i, "uid").map(String::from),
                    },
                )
            })
        })
        .collect()
}

/// An item deleted for good loses its file, if the file is still that item's (it carries its id).
/// Returns how many files went.
pub fn remove_deleted(
    dir: &Path,
    before: &Known,
    live: &HashSet<i64>,
    uid_of: impl Fn(&str) -> Option<String>,
) -> usize {
    let mut removed = 0;
    for (id, was) in before {
        if live.contains(id) || was.uid.is_none() {
            continue;
        }
        let path = dir.join(&was.rel);
        let mine = fs::read_to_string(&path).is_ok_and(|t| uid_of(&t) == was.uid);
        if mine && fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Delete every item of these kinds, for when the folder is changed: nothing of the old one is
/// kept to be written into the new. An item that went with its parent is not an error.
pub fn forget(db: &mut Db, kinds: &[&str]) -> Result<()> {
    for kind in kinds {
        for i in db.list(&Query {
            kind: Some((*kind).into()),
            ..Query::default()
        })? {
            match db.delete(i.id) {
                Ok(_) | Err(Error::NotFound(_)) => {}
                Err(e) => return Err(e),
            }
        }
    }
    Ok(())
}

// ---- conflicts -------------------------------------------------------------------------------

/// Where the app's own version of a file is kept when the file and the item both changed: beside
/// the file, as `<name> (conflict).md` (an `index.md` is named for its directory, beside that).
pub fn conflict_path(vault: &Path, rel: &str, fallback: &str) -> PathBuf {
    let path = vault.join(rel);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(fallback)
        .to_string();
    let (dir, stem) = if stem == "index" {
        let d = path.parent().unwrap_or(vault);
        (
            d.parent().unwrap_or(vault).to_path_buf(),
            d.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(fallback)
                .to_string(),
        )
    } else {
        (path.parent().unwrap_or(vault).to_path_buf(), stem)
    };
    let mut copy = dir.join(format!("{stem} (conflict).md"));
    let mut n = 1;
    while copy.exists() {
        n += 1;
        copy = dir.join(format!("{stem} (conflict {n}).md"));
    }
    copy
}

/// The text without the line that names its id, so the copy is a new item, not a claim.
pub fn without_id(text: &str, key: &str) -> String {
    let prefix = format!("{key}: ");
    let mut out: Vec<&str> = text.lines().filter(|l| !l.starts_with(&prefix)).collect();
    out.push("");
    out.join("\n")
}
