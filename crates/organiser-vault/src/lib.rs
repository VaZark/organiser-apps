//! # organiser-vault
//!
//! What the organiser apps share to keep their items as Markdown files in a directory the person
//! chooses (a "vault"): [`front_matter`] for the block at the top of a file, and the plain file
//! helpers (names that are safe, atomic writes that follow links, small TOML files, the
//! [`Report`] of a sync). What a directory or a file *means* is each app's own.

pub mod front_matter;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use organiser_engine::{Error, Result};

/// Files bigger than this are left alone.
pub const MAX_FILE: u64 = 4 * 1024 * 1024;
/// How deep a vault is read: a guard against loops and runaway trees.
pub const MAX_DEPTH: usize = 32;

/// What a sync or a flush did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    /// Notes made from files that were new.
    pub created: usize,
    /// Notes changed to match their file.
    pub updated: usize,
    /// Notes moved because their file was.
    pub moved: usize,
    /// Notes sent away because their file was gone.
    pub trashed: usize,
    /// Files written for notes changed in the app.
    pub written: usize,
    /// Files deleted for notes deleted in the app.
    pub removed: usize,
    /// Notes whose file and note had both changed: the file won, the note is kept beside it.
    pub conflicts: usize,
    /// Things that were skipped, and why.
    pub warnings: Vec<String>,
}

impl Report {
    pub fn changed(&self) -> bool {
        self.created + self.updated + self.moved + self.trashed + self.written + self.removed + self.conflicts > 0
    }

    pub fn add(&mut self, o: Report) {
        self.created += o.created;
        self.updated += o.updated;
        self.moved += o.moved;
        self.trashed += o.trashed;
        self.written += o.written;
        self.removed += o.removed;
        self.conflicts += o.conflicts;
        self.warnings.extend(o.warnings);
    }

    /// One line for a toast: what changed, or that nothing did.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        for (n, what) in [
            (self.created, "new"),
            (self.updated, "updated"),
            (self.moved, "moved"),
            (self.trashed, "sent away"),
            (self.written, "written"),
            (self.removed, "deleted"),
            (self.conflicts, "in conflict"),
        ] {
            if n > 0 {
                parts.push(format!("{n} {what}"));
            }
        }
        if parts.is_empty() {
            "Everything is up to date".into()
        } else {
            format!("Synced: {}", parts.join(", "))
        }
    }
}


impl Report {
    /// As [`summary`](Self::summary), with `gone` for the items whose file went (`"sent to the
    /// trash"`, `"archived"`).
    pub fn summary_with(&self, gone: &str) -> String {
        self.summary().replace("sent away", gone)
    }
}

pub fn io(e: std::io::Error, path: &Path) -> Error {
    Error::Invalid(format!("{}: {e}", path.display()))
}

/// FNV-1a: a hash that stays the same from one version of Rust to the next.
pub fn hash(text: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// A title as a file or directory name: no separators or characters a file system objects to, no
/// leading dot (it would hide), never empty.
pub fn sanitise(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.').trim();
    let cut: String = cleaned.chars().take(100).collect();
    let cut = cut.trim().to_string();
    if cut.is_empty() { "Untitled".into() } else { cut }
}

/// `base`, or `base-2`, `base-3`, … the first one not in `taken` (compared without case).
pub fn unique(base: &str, taken: &mut HashSet<String>) -> String {
    let mut name = base.to_string();
    let mut n = 1;
    while taken.contains(&name.to_lowercase()) {
        n += 1;
        name = format!("{base}-{n}");
    }
    taken.insert(name.to_lowercase());
    name
}

pub fn is_markdown(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
}

pub fn is_symlink(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink())
}

#[cfg(unix)]
pub fn link(target: &Path, at: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, at)
}

#[cfg(not(unix))]
pub fn link(_: &Path, _: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("links are not supported here"))
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

pub fn rel_of(dir: &Path, stem: &str) -> String {
    dir.join(format!("{stem}.md")).to_string_lossy().replace('\\', "/")
}

pub fn read_dir(p: &Path) -> Vec<fs::DirEntry> {
    fs::read_dir(p).map(|r| r.flatten().collect()).unwrap_or_default()
}

/// Write `text` to `path` without leaving it half written. A link is written through, to the file
/// it points at.
pub fn write_atomic(path: &Path, text: &str) -> Result<()> {
    let target = if is_symlink(path) {
        fs::canonicalize(path).map_err(|e| io(e, path))?
    } else {
        path.to_path_buf()
    };
    if let Some(p) = target.parent() {
        fs::create_dir_all(p).map_err(|e| io(e, p))?;
    }
    let name = target.file_name().and_then(|n| n.to_str()).unwrap_or("note");
    let tmp = target.with_file_name(format!(".{name}.tmp"));
    fs::write(&tmp, text).map_err(|e| io(e, &tmp))?;
    if fs::rename(&tmp, &target).is_err() {
        let _ = fs::remove_file(&tmp);
        fs::write(&target, text).map_err(|e| io(e, &target))?;
    }
    Ok(())
}

/// Remove the directories no folder has (`meta` is the small file each holds of its own), once they hold nothing but what this app writes, deepest
/// first.
pub fn prune(root: &Path, rel: &Path, depth: usize, keep: &HashSet<PathBuf>, meta: &str) {
    if depth > MAX_DEPTH {
        return;
    }
    for e in read_dir(&root.join(rel)) {
        let name = e.file_name();
        let n = name.to_string_lossy();
        if !e.file_type().is_ok_and(|t| t.is_dir()) || n.starts_with('.') {
            continue;
        }
        let child = rel.join(&name);
        prune(root, &child, depth + 1, keep, meta);
        if keep.contains(&child) {
            continue;
        }
        let path = root.join(&child);
        let only_ours = read_dir(&path).iter().all(|f| {
            let fname = f.file_name().to_string_lossy().to_string();
            fname == meta || (fname.starts_with('.') && fname.ends_with(".tmp"))
        });
        if only_ours {
            let _ = fs::remove_file(path.join(meta));
            let _ = fs::remove_dir(&path);
        }
    }
}

// ---- `.folder.toml`: `key = "value"` lines, the little of TOML these files need ----

pub fn read_toml(path: &Path) -> Vec<(String, String)> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.starts_with('#') {
                return None;
            }
            let (k, v) = l.split_once('=')?;
            let v = v.trim();
            let v = match v.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
                Some(inner) => unescape(inner),
                None => v.to_string(),
            };
            Some((k.trim().to_string(), v))
        })
        .collect()
}

pub fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => out.push('\n'),
                Some(o) => out.push(o),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn write_toml(path: &Path, pairs: &[(&str, String)]) -> Result<()> {
    let mut text = String::new();
    for (k, v) in pairs {
        if *v == "true" || *v == "false" {
            text.push_str(&format!("{k} = {v}\n"));
        } else {
            let esc = v.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");
            text.push_str(&format!("{k} = \"{esc}\"\n"));
        }
    }
    if fs::read_to_string(path).is_ok_and(|old| old == text) {
        return Ok(());
    }
    write_atomic(path, &text)
}

