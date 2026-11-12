//! The vault: the notes as Markdown files in a directory the person chooses, as QOwnNotes and
//! Obsidian keep them. The files are the notes; the database is an index of them that keeps what a
//! file cannot (the ids Sticky links to, the order of things, the sticky itself).
//!
//! ```text
//! <vault>/Note in no folder.md       the default folder, "Unassigned", is the vault itself
//! <vault>/Work/Plan.md               a folder is a directory
//! <vault>/Work/Design/Drafts/A.md    a folder inside a folder, to any depth
//! <vault>/Work/.folder.toml          what a directory cannot say: description, colour, icon, ratings
//! <vault>/.trash/Old.md              the trash, with where each note was
//! ```
//!
//! Two directions, each run when the app starts, when its window is focused and on request
//! (there is no file watcher):
//!
//! * [`Vault::sync`] reads the files: new ones become notes, changed ones update their note, moved
//!   ones move it, missing ones send it to the trash.
//! * [`Vault::flush`] writes the notes: every change made in the app reaches its file (a rename
//!   or a move included), and a note that is deleted for good loses its file.
//!
//! Sync always runs first, so what was changed outside is never overwritten. If a file and its
//! note were both changed since they last agreed, the file wins and the note's version is kept
//! beside it as `<name> (conflict).md`.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use organiser_engine::{Db, Error, Item, ItemPatch, Query, Result};
use serde_json::Value;

use super::lifecycle::{self, NewNote};
use crate::primitives::color::Color;
use crate::primitives::folder::{Folder, FolderEdit};
use crate::primitives::md_note::{FileNote, parse_file, render_file};
use crate::primitives::note::{Edit, Note};
use crate::{FOLDER, NOTE};

/// The directory of trashed notes.
pub const TRASH: &str = ".trash";
/// Files bigger than this are left alone.
const MAX_FILE: u64 = 4 * 1024 * 1024;
const FOLDER_META: &str = ".folder.toml";
/// How deep the vault is read: a guard against loops and runaway trees.
const MAX_DEPTH: usize = 32;

/// A directory of notes.
#[derive(Debug, Clone)]
pub struct Vault {
    dir: PathBuf,
}

/// What a [`Vault::sync`] or [`Vault::flush`] did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    /// Notes made from files that were new.
    pub created: usize,
    /// Notes changed to match their file.
    pub updated: usize,
    /// Notes moved because their file was.
    pub moved: usize,
    /// Notes sent to the trash because their file was gone.
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
            (self.trashed, "sent to the trash"),
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

fn io(e: std::io::Error, path: &Path) -> Error {
    Error::Invalid(format!("{}: {e}", path.display()))
}

/// FNV-1a: a hash that stays the same from one version of Rust to the next.
fn hash(text: &str) -> String {
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
fn unique(base: &str, taken: &mut HashSet<String>) -> String {
    let mut name = base.to_string();
    let mut n = 1;
    while taken.contains(&name.to_lowercase()) {
        n += 1;
        name = format!("{base}-{n}");
    }
    taken.insert(name.to_lowercase());
    name
}

fn field<'a>(item: &'a Item, key: &str) -> Option<&'a str> {
    item.fields.get(key).and_then(Value::as_str).filter(|s| !s.is_empty())
}

fn is_markdown(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
}

fn is_symlink(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink())
}

/// The names of the directories, from the folders in the database.
struct Layout {
    /// The directory of every folder, relative to the vault (empty for the default one).
    folder_dir: HashMap<i64, PathBuf>,
    by_dir: HashMap<PathBuf, i64>,
    default: Option<i64>,
}

impl Layout {
    fn load(db: &Db) -> Result<Self> {
        let folders: Vec<Folder> = db
            .list(&Query {
                kind: Some(FOLDER.into()),
                ..Query::default()
            })?
            .iter()
            .map(Folder::from)
            .collect();
        let mut l = Layout {
            folder_dir: HashMap::new(),
            by_dir: HashMap::new(),
            default: folders.iter().find(|f| f.is_default).map(|f| f.id),
        };
        // The default one is the vault itself. The others are named from their titles, made
        // unique among the folders they are with, the first by id keeping a name.
        let mut children: HashMap<Option<i64>, Vec<&Folder>> = HashMap::new();
        for f in folders.iter().filter(|f| !f.is_default) {
            children.entry(f.parent).or_default().push(f);
        }
        for v in children.values_mut() {
            v.sort_by_key(|f| f.id);
        }
        if let Some(d) = l.default {
            l.folder_dir.insert(d, PathBuf::new());
            l.by_dir.insert(PathBuf::new(), d);
        }
        let mut stack: Vec<(Option<i64>, PathBuf, usize)> = vec![(None, PathBuf::new(), 0)];
        while let Some((parent, dir, depth)) = stack.pop() {
            let mut used = HashSet::new();
            for f in children.get(&parent).map(Vec::as_slice).unwrap_or_default() {
                let mine = dir.join(unique(&sanitise(&f.title), &mut used));
                l.folder_dir.insert(f.id, mine.clone());
                l.by_dir.insert(mine.clone(), f.id);
                if depth < MAX_DEPTH {
                    stack.push((Some(f.id), mine, depth + 1));
                }
            }
        }
        Ok(l)
    }

    /// The directory (relative to the vault) the notes of this folder live in.
    fn dir_of(&self, folder: Option<i64>) -> PathBuf {
        folder.and_then(|f| self.folder_dir.get(&f)).cloned().unwrap_or_default()
    }

    /// The path of a folder, for a note in the trash to remember where it was: `Work/Design`.
    fn from_of(&self, folder: Option<i64>) -> Option<String> {
        let d = self.folder_dir.get(&folder?).filter(|d| !d.as_os_str().is_empty())?;
        Some(d.to_string_lossy().replace('\\', "/"))
    }
}

/// A file found in the vault.
struct Found {
    /// Relative to the vault, with `/`.
    rel: String,
    hash: String,
    note: FileNote,
    /// The directory it is in, relative to the vault (empty for the vault itself).
    dir: PathBuf,
    trashed: bool,
    /// The `id` it named, when another file has it: this one is a note of its own.
    lost: Option<i64>,
}

/// The directories found, relative to the vault, parents before the directories in them.
type Dirs = Vec<PathBuf>;

impl Vault {
    /// A vault in `dir`, which is made if it is not there. `db_path` is the database in use: a
    /// vault must not contain it.
    pub fn open(dir: impl Into<PathBuf>, db_path: Option<&Path>) -> Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir).map_err(|e| io(e, &dir))?;
        let dir = fs::canonicalize(&dir).map_err(|e| io(e, &dir))?;
        if let Some(db) = db_path.and_then(|p| fs::canonicalize(p).ok())
            && db.starts_with(&dir)
        {
            return Err(Error::Invalid(format!(
                "{} holds the database; pick another folder for the notes",
                dir.display()
            )));
        }
        Ok(Self { dir })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The file of a note, if it has one.
    pub fn file_of(&self, db: &Db, id: i64) -> Option<PathBuf> {
        let item = db.get(id).ok()?;
        field(&item, "file").map(|f| self.dir.join(f))
    }

    // ---- reading the files into the notes ---------------------------------------------------

    /// Bring the notes up to date with the files.
    pub fn sync(&self, db: &mut Db) -> Result<Report> {
        let mut rep = Report::default();
        lifecycle::ensure_default_folder(db)?;
        let (mut found, dirs) = self.scan(&mut rep);

        // A directory is a folder, and a directory in it a folder in it, to any depth: made when
        // it is new (from the top down), and its `.folder.toml` says the rest.
        let mut layout = Layout::load(db)?;
        for rel in &dirs {
            let meta = self.dir.join(rel).join(FOLDER_META);
            if rel.as_os_str().is_empty() {
                self.read_folder_meta(db, layout.default.unwrap_or_default(), &meta)?;
                continue;
            }
            let id = match layout.by_dir.get(rel) {
                Some(id) => *id,
                None => {
                    let name = rel.file_name().and_then(|n| n.to_str()).unwrap_or("Folder").to_string();
                    let title = self.meta(&meta, "title").unwrap_or(name);
                    let parent = rel
                        .parent()
                        .filter(|p| !p.as_os_str().is_empty())
                        .and_then(|p| layout.by_dir.get(p).copied());
                    lifecycle::create_folder_in(db, parent, &title, false)?.id
                }
            };
            self.read_folder_meta(db, id, &meta)?;
            layout = Layout::load(db)?;
        }

        // Two files claiming one note: the one that comes first by path keeps it, then the older
        // (by its `created`); the others are notes of their own.
        let mut claims: HashMap<i64, Vec<usize>> = HashMap::new();
        for (i, f) in found.iter().enumerate() {
            if let Some(id) = f.note.id {
                claims.entry(id).or_default().push(i);
            }
        }
        for (_, mut idx) in claims {
            if idx.len() < 2 {
                continue;
            }
            idx.sort_by(|a, b| {
                let (fa, fb) = (&found[*a], &found[*b]);
                fa.rel
                    .to_lowercase()
                    .cmp(&fb.rel.to_lowercase())
                    .then(fa.rel.cmp(&fb.rel))
                    .then(fa.note.created.unwrap_or(i64::MAX).cmp(&fb.note.created.unwrap_or(i64::MAX)))
            });
            for loser in &idx[1..] {
                found[*loser].lost = found[*loser].note.id.take();
            }
        }

        let items: Vec<Item> = self.note_items(db)?;
        let by_id: HashMap<i64, &Item> = items.iter().map(|i| (i.id, i)).collect();
        let mut seen: HashSet<i64> = HashSet::new();
        // A file with no id of its own is the note that has its path (a plain file already read).
        let by_file: HashMap<String, i64> = items
            .iter()
            .filter_map(|i| field(i, "file").map(|f| (f.to_string(), i.id)))
            .collect();
        // Notes not yet tied to a file, to adopt a plain file that is the same note.
        let mut untied: HashMap<(Option<i64>, String), i64> = HashMap::new();
        for i in &items {
            if field(i, "file").is_none() {
                let n = Note::from(i);
                untied.insert((n.folder_id, n.title.clone()), n.id);
            }
        }

        for f in &found {
            let folder = self.place_of(&layout, f);
            let note_id = f
                .note
                .id
                .filter(|id| by_id.contains_key(id) && !seen.contains(id))
                .or_else(|| {
                    // A file with no id: the note that has this path, else one filed where the
                    // file is, with its title.
                    // (An id no note has, from another database, names nothing here.)
                    let named = f.note.id.or(f.lost);
                    if named.is_some_and(|id| by_id.contains_key(&id)) {
                        return None;
                    }
                    by_file
                        .get(&f.rel)
                        .copied()
                        .filter(|id| !seen.contains(id))
                        .or_else(|| untied.remove(&(folder, f.note.title.clone())))
                });
            match note_id {
                Some(id) => {
                    seen.insert(id);
                    let item = db.get(id)?;
                    self.read_into(db, &item, f, folder, &layout, &mut rep)?;
                }
                None => {
                    let id = self.create_from(db, f, folder)?;
                    seen.insert(id);
                    rep.created += 1;
                }
            }
        }

        // A note whose file is gone: to the trash; if it was already there, gone for good.
        for i in &items {
            if seen.contains(&i.id) || field(i, "file").is_none() {
                continue;
            }
            let n = Note::from(i);
            if n.is_trashed() {
                db.delete(i.id)?;
            } else {
                lifecycle::trash(db, i.id, now())?;
                rep.trashed += 1;
            }
        }
        Ok(rep)
    }

    fn note_items(&self, db: &Db) -> Result<Vec<Item>> {
        db.list(&Query {
            kind: Some(NOTE.into()),
            ..Query::default()
        })
    }

    /// The folder a found file belongs to.
    fn place_of(&self, layout: &Layout, f: &Found) -> Option<i64> {
        let default = layout.default;
        if f.trashed {
            // Back to where it was, when that is still there.
            return f
                .note
                .from
                .as_ref()
                .and_then(|from| layout.by_dir.get(Path::new(from)).copied())
                .or(default);
        }
        layout.by_dir.get(&f.dir).copied().or(default)
    }

    fn create_from(&self, db: &mut Db, f: &Found, folder: Option<i64>) -> Result<i64> {
        let rated = match folder {
            Some(fo) => Folder::from(&db.get(fo)?).rated,
            None => false,
        };
        let n = lifecycle::create(
            db,
            &NewNote {
                title: f.note.title.clone(),
                body: f.note.body.clone(),
                color: f.note.color,
                tags: f.note.tags.clone(),
                folder,
                favorite: f.note.favorite,
                rating: f.note.rating.filter(|_| rated),
            },
        )?;
        if f.note.pinned {
            lifecycle::set_pinned(db, n.id, true)?;
        }
        if f.note.archived {
            lifecycle::set_archived(db, n.id, true)?;
        }
        if f.trashed {
            lifecycle::trash(db, n.id, f.note.trashed.unwrap_or_else(now))?;
        }
        self.tie(db, n.id, f, false)?;
        Ok(n.id)
    }

    /// Record that the note is this file as it is now. `again` asks for the file to be written
    /// once more by the next flush (it lacks the note's id, say).
    fn tie(&self, db: &mut Db, id: i64, f: &Found, again: bool) -> Result<()> {
        let item = db.get(id)?;
        let note = Note::from(&item);
        let layout = Layout::load(db)?;
        let sig = if again || f.note.id != Some(id) {
            String::new()
        } else {
            self.sig(&layout, &note)
        };
        db.update(
            id,
            ItemPatch::default()
                .field("file", f.rel.as_str())
                .field("file_hash", f.hash.as_str())
                .field("file_sig", sig),
        )?;
        Ok(())
    }

    /// What the note would be written as, with where: a change to either is a change to the note.
    fn sig(&self, layout: &Layout, n: &Note) -> String {
        let place = if n.is_trashed() {
            PathBuf::from(TRASH)
        } else {
            layout.dir_of(n.folder_id)
        };
        hash(&format!(
            "{}\n{}",
            render_file(n, self.from_for(layout, n).as_deref(), None),
            place.display()
        ))
    }

    fn from_for(&self, layout: &Layout, n: &Note) -> Option<String> {
        n.is_trashed().then(|| layout.from_of(n.folder_id)).flatten()
    }

    /// Make an existing note match its file, if the file has changed (or moved).
    #[allow(clippy::too_many_arguments)]
    fn read_into(
        &self,
        db: &mut Db,
        item: &Item,
        f: &Found,
        folder: Option<i64>,
        layout: &Layout,
        rep: &mut Report,
    ) -> Result<()> {
        let note = Note::from(item);
        let stored = field(item, "file_hash").unwrap_or("");
        let sig_now = self.sig(layout, &note);
        let row_changed = field(item, "file_sig").is_some_and(|s| s != sig_now);
        let file_changed = stored != f.hash;
        let placed = note.folder_id == folder
            && note.is_trashed() == f.trashed
            && field(item, "file") == Some(f.rel.as_str());
        if !file_changed && placed {
            return Ok(());
        }
        if !file_changed && row_changed {
            // Only the note changed: the flush writes it. (It also moved here, so the file's own
            // place is not the note's: leave both.)
            return Ok(());
        }
        if file_changed && row_changed && field(item, "file_sig").is_some() {
            // Both: the file wins, and the note is kept beside it.
            self.keep_conflict(&note, layout, &f.rel, rep)?;
            rep.conflicts += 1;
        }
        let mut changed = self.apply(db, &note, f, folder)?;
        if !file_changed && !placed {
            rep.moved += usize::from(changed);
            changed = false;
        }
        if changed {
            rep.updated += 1;
        }
        self.tie(db, note.id, f, false)?;
        Ok(())
    }

    /// Put the file's content into the note. True if anything differed.
    fn apply(&self, db: &mut Db, cur: &Note, f: &Found, folder: Option<i64>) -> Result<bool> {
        let id = cur.id;
        let mut changed = false;
        let want = &f.note;
        let edit = Edit {
            title: (cur.title != want.title).then(|| want.title.clone()),
            body: (cur.body != want.body).then(|| want.body.clone()),
            color: (cur.color != want.color).then_some(want.color),
            favorite: (cur.favorite != want.favorite).then_some(want.favorite),
            ..Edit::default()
        };
        if edit.title.is_some() || edit.body.is_some() || edit.color.is_some() || edit.favorite.is_some() {
            lifecycle::edit(db, id, &edit)?;
            changed = true;
        }
        if cur.tags != want.tags {
            lifecycle::set_tags(db, id, &want.tags)?;
            changed = true;
        }
        if cur.folder_id != folder {
            lifecycle::move_note(db, id, folder)?;
            changed = true;
        }
        if cur.pinned != want.pinned && !f.trashed {
            lifecycle::set_pinned(db, id, want.pinned)?;
            changed = true;
        }
        if cur.archived != want.archived {
            lifecycle::set_archived(db, id, want.archived)?;
            changed = true;
        }
        // A rating only where the folder offers them.
        if let Some(fo) = folder
            && Folder::from(&db.get(fo)?).rated
            && cur.rating != want.rating
        {
            lifecycle::edit(
                db,
                id,
                &Edit {
                    rating: Some(want.rating),
                    ..Edit::default()
                },
            )?;
            changed = true;
        }
        match (cur.is_trashed(), f.trashed) {
            (false, true) => {
                lifecycle::trash(db, id, want.trashed.unwrap_or_else(now))?;
                changed = true;
            }
            (true, false) => {
                lifecycle::restore(db, id)?;
                changed = true;
            }
            _ => {}
        }
        Ok(changed)
    }

    /// Save the note's own version next to the file that won, as a note without an id.
    fn keep_conflict(&self, note: &Note, layout: &Layout, rel: &str, rep: &mut Report) -> Result<()> {
        let path = self.dir.join(rel);
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("note").to_string();
        let dir = path.parent().unwrap_or(&self.dir).to_path_buf();
        let mut copy = dir.join(format!("{stem} (conflict).md"));
        let mut n = 1;
        while copy.exists() {
            n += 1;
            copy = dir.join(format!("{stem} (conflict {n}).md"));
        }
        let text = render_file(note, self.from_for(layout, note).as_deref(), None).replacen(&format!("id: {}\n", note.id), "", 1);
        write_atomic(&copy, &text)?;
        rep.warnings.push(format!(
            "{rel} was changed in the app and outside it; the file was kept, the app's version is {}",
            copy.file_name().and_then(|n| n.to_str()).unwrap_or("beside it")
        ));
        Ok(())
    }

    /// Every note file in the vault, and the directories that hold them.
    fn scan(&self, rep: &mut Report) -> (Vec<Found>, Dirs) {
        let mut found = Vec::new();
        let mut dirs: Dirs = Vec::new();
        self.scan_tree(Path::new(""), 0, &mut found, &mut dirs, rep);
        self.scan_files(&self.dir.join(TRASH), Path::new(""), true, &mut found, rep);
        (found, dirs)
    }

    /// `rel` and the directories in it, depth first, each parent before what is in it.
    fn scan_tree(&self, rel: &Path, depth: usize, found: &mut Vec<Found>, dirs: &mut Dirs, rep: &mut Report) {
        dirs.push(rel.to_path_buf());
        self.scan_files(&self.dir.join(rel), rel, false, found, rep);
        if depth >= MAX_DEPTH {
            return;
        }
        let mut subs: Vec<String> = read_dir(&self.dir.join(rel))
            .into_iter()
            // A directory that is really one: a link to somewhere else is not followed.
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| !n.starts_with('.'))
            .collect();
        subs.sort();
        for name in subs {
            self.scan_tree(&rel.join(name), depth + 1, found, dirs, rep);
        }
    }

    fn scan_files(&self, path: &Path, dir: &Path, trashed: bool, out: &mut Vec<Found>, rep: &mut Report) {
        let mut files: Vec<_> = read_dir(path)
            .into_iter()
            .filter(|e| is_markdown(&e.path()) && !e.file_name().to_string_lossy().starts_with('.'))
            .collect();
        files.sort_by_key(|e| e.file_name());
        for e in files {
            let p = e.path();
            let rel = p.strip_prefix(&self.dir).unwrap_or(&p).to_string_lossy().replace('\\', "/");
            // A link to a file is followed; one that leads nowhere is reported.
            let meta = match fs::metadata(&p) {
                Ok(m) if m.is_file() => m,
                Ok(_) => continue,
                Err(_) => {
                    rep.warnings.push(format!("{rel} is a link to a file that is not there"));
                    continue;
                }
            };
            if meta.len() > MAX_FILE {
                rep.warnings.push(format!("{rel} is too big to be a note"));
                continue;
            }
            let Ok(text) = fs::read_to_string(&p) else {
                rep.warnings.push(format!("{rel} is not text"));
                continue;
            };
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("Untitled");
            out.push(Found {
                hash: hash(&text),
                note: parse_file(&text, stem),
                rel,
                dir: dir.to_path_buf(),
                trashed,
                lost: None,
            });
        }
    }

    fn meta(&self, path: &Path, key: &str) -> Option<String> {
        read_toml(path).into_iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    fn read_folder_meta(&self, db: &mut Db, id: i64, path: &Path) -> Result<()> {
        let pairs = read_toml(path);
        if pairs.is_empty() {
            return Ok(());
        }
        let cur = Folder::from(&db.get(id)?);
        let get = |k: &str| pairs.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
        let mut edit = FolderEdit::default();
        if !cur.is_default && let Some(t) = get("title").filter(|t| *t != cur.title) {
            edit.title = Some(t);
        }
        let notes = get("description").unwrap_or_default();
        if notes != cur.notes {
            edit.notes = Some(notes);
        }
        if let Some(r) = get("rated").map(|v| v == "true").filter(|r| *r != cur.rated) {
            edit.rated = Some(r);
        }
        let color = get("color").and_then(|c| Color::parse(&c));
        if color != cur.color {
            edit.color = Some(color);
        }
        let icon = get("icon");
        if icon != cur.icon {
            edit.icon = Some(icon);
        }
        if edit.title.is_some() || edit.notes.is_some() || edit.rated.is_some() || edit.color.is_some() || edit.icon.is_some() {
            lifecycle::edit_folder(db, id, &edit)?;
        }
        Ok(())
    }

    // ---- writing the notes out ---------------------------------------------------------------

    /// The file each note has now, to hand back to [`flush`](Self::flush): that is how a note
    /// deleted for good is known to have had one.
    pub fn known(&self, db: &Db) -> Result<BTreeMap<i64, String>> {
        Ok(self
            .note_items(db)?
            .iter()
            .filter_map(|i| field(i, "file").map(|f| (i.id, f.to_string())))
            .collect())
    }

    /// Bring the files up to date with the notes. `before` is [`known`](Self::known) from before
    /// the changes: a note in it that is gone loses its file.
    pub fn flush(&self, db: &mut Db, before: &BTreeMap<i64, String>) -> Result<Report> {
        let mut rep = Report::default();
        let layout = Layout::load(db)?;
        let items = self.note_items(db)?;
        let live: HashSet<i64> = items.iter().map(|i| i.id).collect();

        // A note deleted for good: its file, if it is still that note's.
        for (id, rel) in before {
            if live.contains(id) {
                continue;
            }
            let path = self.dir.join(rel);
            let mine = fs::read_to_string(&path)
                .map(|t| parse_file(&t, "").id == Some(*id))
                .unwrap_or(false);
            if mine && fs::remove_file(&path).is_ok() {
                rep.removed += 1;
            }
        }

        self.write_meta(db, &layout)?;

        // Names: a note keeps its file name while it still suits its title; the rest are given
        // the first free one, in alphabetical order (then by creation).
        let want = self.plan_names(&layout, &items);
        for item in &items {
            if let Err(e) = self.write_one(db, &layout, item, &want[&item.id], &mut rep) {
                rep.warnings.push(e.to_string());
            }
        }
        let keep: HashSet<PathBuf> = layout.folder_dir.values().cloned().collect();
        prune(&self.dir, Path::new(""), 0, &keep);
        Ok(rep)
    }

    fn write_meta(&self, db: &Db, layout: &Layout) -> Result<()> {
        let folders: Vec<Folder> = db
            .list(&Query {
                kind: Some(FOLDER.into()),
                ..Query::default()
            })?
            .iter()
            .map(Folder::from)
            .collect();
        for f in &folders {
            let Some(dir) = layout.folder_dir.get(&f.id) else { continue };
            let mut pairs = vec![("title", f.title.clone())];
            if !f.notes.is_empty() {
                pairs.push(("description", f.notes.clone()));
            }
            if f.rated {
                pairs.push(("rated", "true".into()));
            }
            if let Some(c) = f.color {
                pairs.push(("color", c.hex()));
            }
            if let Some(i) = &f.icon {
                pairs.push(("icon", i.clone()));
            }
            // The default folder is the vault itself: only worth a file if it says something.
            if f.is_default && pairs.len() == 1 {
                continue;
            }
            write_toml(&self.dir.join(dir).join(FOLDER_META), &pairs)?;
        }
        Ok(())
    }

    /// The relative path each note should have.
    fn plan_names(&self, layout: &Layout, items: &[Item]) -> HashMap<i64, String> {
        let mut taken: HashMap<PathBuf, HashSet<String>> = HashMap::new();
        let mut out: HashMap<i64, String> = HashMap::new();
        let mut later: Vec<(PathBuf, String, &Item)> = Vec::new();
        for i in items {
            let n = Note::from(i);
            let dir = if n.is_trashed() {
                PathBuf::from(TRASH)
            } else {
                layout.dir_of(n.folder_id)
            };
            let base = sanitise(&n.title);
            // Keep the file it has when it is in the right place and still named for the title.
            let keeps = field(i, "file").and_then(|f| {
                let p = Path::new(f);
                let stem = p.file_stem()?.to_str()?.to_string();
                let suits = stem == base
                    || stem
                        .strip_prefix(&format!("{base}-"))
                        .is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty());
                (p.parent() == Some(dir.as_path()) && suits).then_some(stem)
            });
            match keeps {
                Some(stem) if taken.entry(dir.clone()).or_default().insert(stem.to_lowercase()) => {
                    out.insert(i.id, rel_of(&dir, &stem));
                }
                _ => later.push((dir, base, i)),
            }
        }
        later.sort_by(|a, b| {
            a.1.to_lowercase()
                .cmp(&b.1.to_lowercase())
                .then(a.2.created.cmp(&b.2.created))
                .then(a.2.id.cmp(&b.2.id))
        });
        for (dir, base, i) in later {
            let name = unique(&base, taken.entry(dir.clone()).or_default());
            out.insert(i.id, rel_of(&dir, &name));
        }
        out
    }

    fn write_one(&self, db: &mut Db, layout: &Layout, item: &Item, want: &str, rep: &mut Report) -> Result<()> {
        let note = Note::from(item);
        let recorded = field(item, "file").map(String::from);
        let old_path = recorded.as_ref().map(|r| self.dir.join(r));
        let on_disk = old_path.as_ref().and_then(|p| fs::read_to_string(p).ok());
        // The file was changed outside since they last agreed: the next sync takes it in.
        if let (Some(text), Some(stored)) = (&on_disk, field(item, "file_hash"))
            && hash(text) != stored
        {
            return Ok(());
        }
        let sig = self.sig(layout, &note);
        if recorded.as_deref() == Some(want) && field(item, "file_sig") == Some(sig.as_str()) && on_disk.is_some() {
            return Ok(());
        }
        // A name taken by a file that is not this note's (one not read yet) is not overwritten.
        let mut want = want.to_string();
        let mut n = 1;
        while self.dir.join(&want).exists() && recorded.as_deref() != Some(want.as_str()) {
            n += 1;
            let p = Path::new(&want);
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("note");
            want = rel_of(p.parent().unwrap_or(Path::new("")), &format!("{stem}-{n}"));
        }
        let previous = on_disk.as_deref().map(|t| parse_file(t, "").front).unwrap_or(None);
        let text = render_file(&note, self.from_for(layout, &note).as_deref(), previous.as_ref());
        let new_path = self.dir.join(&want);
        if let Some(p) = new_path.parent() {
            fs::create_dir_all(p).map_err(|e| io(e, p))?;
        }
        // Moved or renamed: the file (or the link to it) goes first, then it is written.
        if let Some(old) = &old_path
            && old != &new_path
            && old.exists()
        {
            fs::rename(old, &new_path).map_err(|e| io(e, old))?;
        }
        write_atomic(&new_path, &text)?;
        db.update(
            note.id,
            ItemPatch::default()
                .field("file", want.as_str())
                .field("file_hash", hash(&text))
                .field("file_sig", sig),
        )?;
        rep.written += 1;
        Ok(())
    }

    // ---- bringing a file in -----------------------------------------------------------------

    /// Put the Markdown file `src` into the vault, in `folder` (the default folder
    /// if none), so the app and any editor work on the one file. It is moved; if it cannot be
    /// (another file system, say) a link to it is put in the vault instead. Then call
    /// [`sync`](Self::sync). Returns the new path and whether it is a link.
    pub fn add_file(&self, db: &Db, src: &Path, folder: Option<i64>) -> Result<(PathBuf, bool)> {
        if !src.is_file() || !is_markdown(src) {
            return Err(Error::Invalid(format!("{} is not a Markdown file", src.display())));
        }
        let layout = Layout::load(db)?;
        let dir = self.dir.join(layout.dir_of(folder.or(layout.default)));
        fs::create_dir_all(&dir).map_err(|e| io(e, &dir))?;
        let stem = src.file_stem().and_then(|s| s.to_str()).map_or_else(|| "Untitled".to_string(), sanitise);
        let mut dest = dir.join(format!("{stem}.md"));
        let mut n = 1;
        while dest.exists() || is_symlink(&dest) {
            n += 1;
            dest = dir.join(format!("{stem}-{n}.md"));
        }
        match fs::rename(src, &dest) {
            Ok(()) => Ok((dest, false)),
            Err(_) => {
                let target = fs::canonicalize(src).map_err(|e| io(e, src))?;
                link(&target, &dest).map_err(|e| io(e, &dest))?;
                Ok((dest, true))
            }
        }
    }
}

#[cfg(unix)]
fn link(target: &Path, at: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, at)
}

#[cfg(not(unix))]
fn link(_: &Path, _: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("links are not supported here"))
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn rel_of(dir: &Path, stem: &str) -> String {
    dir.join(format!("{stem}.md")).to_string_lossy().replace('\\', "/")
}

fn read_dir(p: &Path) -> Vec<fs::DirEntry> {
    fs::read_dir(p).map(|r| r.flatten().collect()).unwrap_or_default()
}

/// Write `text` to `path` without leaving it half written. A link is written through, to the file
/// it points at.
fn write_atomic(path: &Path, text: &str) -> Result<()> {
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

/// Remove the directories no folder has, once they hold nothing but what this app writes, deepest
/// first.
fn prune(root: &Path, rel: &Path, depth: usize, keep: &HashSet<PathBuf>) {
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
        prune(root, &child, depth + 1, keep);
        if keep.contains(&child) {
            continue;
        }
        let path = root.join(&child);
        let only_ours = read_dir(&path).iter().all(|f| {
            let fname = f.file_name().to_string_lossy().to_string();
            fname == FOLDER_META || (fname.starts_with('.') && fname.ends_with(".tmp"))
        });
        if only_ours {
            let _ = fs::remove_file(path.join(FOLDER_META));
            let _ = fs::remove_dir(&path);
        }
    }
}

// ---- `.folder.toml`: `key = "value"` lines, the little of TOML these files need ----

fn read_toml(path: &Path) -> Vec<(String, String)> {
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

fn unescape(s: &str) -> String {
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

fn write_toml(path: &Path, pairs: &[(&str, String)]) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SCHEMA;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static N: AtomicUsize = AtomicUsize::new(0);

    /// A fresh empty directory for one test.
    fn temp() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "notes-vault-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn setup() -> (Db, Vault, PathBuf) {
        let mut db = Db::open_in_memory(&SCHEMA).unwrap();
        lifecycle::ensure_default_folder(&mut db).unwrap();
        let dir = temp();
        let v = Vault::open(&dir, None).unwrap();
        (db, v, dir)
    }

    fn sync_flush(db: &mut Db, v: &Vault) -> Report {
        let before = v.known(db).unwrap();
        let mut r = v.sync(db).unwrap();
        r.add(v.flush(db, &before).unwrap());
        r
    }

    fn read(dir: &Path, rel: &str) -> String {
        fs::read_to_string(dir.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
    }

    fn titles(db: &Db) -> Vec<String> {
        let mut t: Vec<String> = db
            .list(&Query { kind: Some(NOTE.into()), ..Query::default() })
            .unwrap()
            .iter()
            .map(|i| i.title.clone())
            .collect();
        t.sort();
        t
    }

    #[test]
    fn a_note_becomes_a_file_in_its_folder_and_in_a_folder_in_it() {
        let (mut db, v, dir) = setup();
        let work = lifecycle::create_folder(&mut db, "Work", false).unwrap();
        let design = lifecycle::create_folder_in(&mut db, Some(work.id), "Design", false).unwrap();
        let a = lifecycle::create(&mut db, &NewNote { title: "Plan".into(), body: "text".into(), folder: Some(work.id), ..NewNote::default() }).unwrap();
        let b = lifecycle::create(&mut db, &NewNote { title: "Wires".into(), folder: Some(design.id), ..NewNote::default() }).unwrap();
        lifecycle::create(&mut db, &NewNote::new("Loose")).unwrap();
        let r = v.flush(&mut db, &BTreeMap::new()).unwrap();
        assert_eq!(r.written, 3);
        assert!(read(&dir, "Work/Plan.md").contains("# Plan\n\ntext\n"));
        assert!(read(&dir, "Work/Plan.md").starts_with(&format!("---\nid: {}\n", a.id)));
        assert!(dir.join("Work/Design/Wires.md").exists());
        assert!(dir.join("Loose.md").exists(), "the default folder is the vault itself");
        assert!(dir.join("Work/.folder.toml").exists());
        let _ = b;
        // Nothing changed: nothing written.
        let known = v.known(&db).unwrap();
        assert_eq!(v.flush(&mut db, &known).unwrap().written, 0);
    }

    #[test]
    fn a_new_file_becomes_a_note_with_a_folder_for_each_directory_at_any_depth() {
        let (mut db, v, dir) = setup();
        fs::create_dir_all(dir.join("Ideas/Later")).unwrap();
        fs::create_dir_all(dir.join("Ideas/Later/too/deep")).unwrap();
        fs::write(dir.join("Top.md"), "# Top\n\nhello\n").unwrap();
        fs::write(dir.join("Ideas/Spark.md"), "---\ntags: [a, b]\npinned: true\n---\n# Spark\n\nbody\n").unwrap();
        fs::write(dir.join("Ideas/Later/Maybe.md"), "no heading here\n").unwrap();
        fs::write(dir.join("Ideas/Later/too/Deep.md"), "# Deep\n").unwrap();
        fs::write(dir.join("Ideas/readme.txt"), "not a note").unwrap();
        let r = sync_flush(&mut db, &v);
        assert_eq!(r.created, 4, "{r:?}");
        assert_eq!(titles(&db), ["Deep", "Maybe", "Spark", "Top"]);
        let items = v.note_items(&db).unwrap();
        let spark = Note::from(items.iter().find(|i| i.title == "Spark").unwrap());
        assert_eq!(spark.tags, ["a", "b"]);
        assert!(spark.pinned);
        let ideas = crate::views::folders(&db).unwrap().into_iter().find(|f| f.folder.title == "Ideas").unwrap();
        assert_eq!(spark.folder_id, Some(ideas.folder.id));
        // Ideas > Later > Maybe, Ideas > Later > too > Deep, and the empty Ideas > Later > too > deep.
        let all = crate::views::folders(&db).unwrap();
        let by = |t: &str| all.iter().find(|f| f.folder.title == t).unwrap().folder.clone();
        let (later, too, deep) = (by("Later"), by("too"), by("deep"));
        assert_eq!(later.parent, Some(ideas.folder.id));
        assert_eq!((too.parent, deep.parent), (Some(later.id), Some(too.id)));
        let maybe = Note::from(items.iter().find(|i| i.title == "Maybe").unwrap());
        assert_eq!(maybe.folder_id, Some(later.id));
        let deepnote = Note::from(items.iter().find(|i| i.title == "Deep").unwrap());
        assert_eq!(deepnote.folder_id, Some(too.id));
        let _ = deep; // an empty directory is a folder too
        // Files that had no id got one when written back.
        assert!(read(&dir, "Top.md").contains("id: "));
        // Running it again changes nothing.
        assert!(!sync_flush(&mut db, &v).changed());
        assert_eq!(titles(&db).len(), 4);
    }

    #[test]
    fn plain_files_are_not_read_twice_before_they_are_written() {
        // Two reads with no write between (the command line reads, runs, then writes).
        let (mut db, v, dir) = setup();
        fs::write(dir.join("Plain.md"), "just text\n").unwrap();
        fs::write(dir.join("Titled.md"), "# Titled\n\nbody\n").unwrap();
        assert_eq!(v.sync(&mut db).unwrap().created, 2);
        let r = v.sync(&mut db).unwrap();
        assert!(!r.changed(), "{r:?}");
        assert_eq!(titles(&db), ["Plain", "Titled"]);
        // And a note whose file is edited before its first write is that note, not a new one.
        fs::write(dir.join("Plain.md"), "changed text\n").unwrap();
        let r = v.sync(&mut db).unwrap();
        assert_eq!((r.created, r.updated), (0, 1), "{r:?}");
        assert_eq!(titles(&db).len(), 2);
    }

    #[test]
    fn files_with_an_id_no_note_has_are_not_read_twice_either() {
        let (mut db, v, dir) = setup();
        let text = "---\nid: 7001\n---\n# Dup\n\nsame\n";
        fs::write(dir.join("A.md"), text).unwrap();
        fs::write(dir.join("B.md"), text).unwrap();
        fs::write(dir.join("C.md"), "---\nid: 555\n---\n# Stranger\n").unwrap();
        assert_eq!(v.sync(&mut db).unwrap().created, 3);
        let r = v.sync(&mut db).unwrap();
        assert!(!r.changed(), "{r:?}");
        assert_eq!(titles(&db), ["Dup", "Dup", "Stranger"]);
    }

    #[test]
    fn a_file_changed_outside_updates_its_note() {
        let (mut db, v, dir) = setup();
        let n = lifecycle::create(&mut db, &NewNote { title: "Doc".into(), body: "one".into(), ..NewNote::default() }).unwrap();
        sync_flush(&mut db, &v);
        let text = read(&dir, "Doc.md").replace("one", "two");
        fs::write(dir.join("Doc.md"), text).unwrap();
        let r = sync_flush(&mut db, &v);
        assert_eq!(r.updated, 1, "{r:?}");
        assert_eq!(Note::from(&db.get(n.id).unwrap()).body, "two");
    }

    #[test]
    fn a_change_in_the_app_reaches_the_file_and_a_rename_moves_it() {
        let (mut db, v, dir) = setup();
        let n = lifecycle::create(&mut db, &NewNote { title: "Doc".into(), body: "one".into(), ..NewNote::default() }).unwrap();
        sync_flush(&mut db, &v);
        lifecycle::edit(&mut db, n.id, &Edit { title: Some("Renamed".into()), body: Some("two".into()), ..Edit::default() }).unwrap();
        let r = sync_flush(&mut db, &v);
        assert_eq!(r.written, 1, "{r:?}");
        assert!(!dir.join("Doc.md").exists());
        assert!(read(&dir, "Renamed.md").contains("two"));
        // A move to a folder moves the file.
        let f = lifecycle::create_folder(&mut db, "Box", false).unwrap();
        lifecycle::move_note(&mut db, n.id, f.id).unwrap();
        sync_flush(&mut db, &v);
        assert!(dir.join("Box/Renamed.md").exists());
        assert!(!dir.join("Renamed.md").exists());
        // Renaming the folder renames the directory and the empty one goes.
        lifecycle::edit_folder(&mut db, f.id, &FolderEdit { title: Some("Crate".into()), ..FolderEdit::default() }).unwrap();
        // The app flushes after every change, before anything is read again.
        let known = v.known(&db).unwrap();
        v.flush(&mut db, &known).unwrap();
        assert!(!v.sync(&mut db).unwrap().changed());
        assert!(dir.join("Crate/Renamed.md").exists());
        assert!(!dir.join("Box").exists());
    }

    #[test]
    fn a_file_moved_between_directories_moves_its_note() {
        let (mut db, v, dir) = setup();
        let a = lifecycle::create_folder(&mut db, "A", false).unwrap();
        let b = lifecycle::create_folder(&mut db, "B", false).unwrap();
        let n = lifecycle::create(&mut db, &NewNote { title: "Mover".into(), folder: Some(a.id), ..NewNote::default() }).unwrap();
        sync_flush(&mut db, &v);
        fs::rename(dir.join("A/Mover.md"), dir.join("B/Mover.md")).unwrap();
        let r = sync_flush(&mut db, &v);
        assert_eq!(r.moved, 1, "{r:?}");
        assert_eq!(Note::from(&db.get(n.id).unwrap()).folder_id, Some(b.id));
        assert_eq!(titles(&db), ["Mover"], "no copy was made");
    }

    #[test]
    fn a_deleted_file_sends_its_note_to_the_trash_and_a_second_time_deletes_it() {
        let (mut db, v, dir) = setup();
        let n = lifecycle::create(&mut db, &NewNote::new("Gone")).unwrap();
        sync_flush(&mut db, &v);
        fs::remove_file(dir.join("Gone.md")).unwrap();
        let r = sync_flush(&mut db, &v);
        assert_eq!(r.trashed, 1, "{r:?}");
        assert!(Note::from(&db.get(n.id).unwrap()).is_trashed());
        assert!(dir.join(".trash/Gone.md").exists(), "kept in the vault's trash");
        // Restored in the app: back out of the trash.
        lifecycle::restore(&mut db, n.id).unwrap();
        sync_flush(&mut db, &v);
        assert!(dir.join("Gone.md").exists());
        assert!(!dir.join(".trash/Gone.md").exists());
        // Deleted for good in the app: the file goes.
        lifecycle::trash(&mut db, n.id, 5).unwrap();
        sync_flush(&mut db, &v);
        assert!(dir.join(".trash/Gone.md").exists());
        let before = v.known(&db).unwrap();
        lifecycle::purge(&mut db, n.id).unwrap();
        let r = v.flush(&mut db, &before).unwrap();
        assert_eq!(r.removed, 1);
        assert!(!dir.join(".trash/Gone.md").exists());
    }

    #[test]
    fn a_trashed_note_remembers_where_it_was() {
        let (mut db, v, dir) = setup();
        let f = lifecycle::create_folder(&mut db, "Work", false).unwrap();
        let s = lifecycle::create_folder_in(&mut db, Some(f.id), "Design", false).unwrap();
        let n = lifecycle::create(&mut db, &NewNote { title: "Old".into(), folder: Some(s.id), ..NewNote::default() }).unwrap();
        sync_flush(&mut db, &v);
        lifecycle::trash(&mut db, n.id, 100).unwrap();
        sync_flush(&mut db, &v);
        assert!(read(&dir, ".trash/Old.md").contains("from: Work/Design"));
        // Restoring it from outside, by moving the file back, puts it there.
        let text = read(&dir, ".trash/Old.md").replace("trashed: 1970-01-01T00:01:40Z\n", "");
        fs::remove_file(dir.join(".trash/Old.md")).unwrap();
        fs::write(dir.join("Work/Design/Old.md"), text).unwrap();
        sync_flush(&mut db, &v);
        let now = Note::from(&db.get(n.id).unwrap());
        assert!(!now.is_trashed());
        assert_eq!(now.folder_id, Some(s.id));
    }

    #[test]
    fn folders_nest_to_any_depth_and_a_folder_in_the_middle_takes_what_is_below_it_along() {
        let (mut db, v, dir) = setup();
        let a = lifecycle::create_folder(&mut db, "A", false).unwrap();
        let b = lifecycle::create_folder_in(&mut db, Some(a.id), "B", false).unwrap();
        let c = lifecycle::create_folder_in(&mut db, Some(b.id), "C", false).unwrap();
        let d = lifecycle::create_folder_in(&mut db, Some(c.id), "D", false).unwrap();
        for (t, f) in [("One", a.id), ("Two", b.id), ("Three", c.id), ("Four", d.id)] {
            lifecycle::create(&mut db, &NewNote { title: t.into(), folder: Some(f), ..NewNote::default() }).unwrap();
        }
        sync_flush(&mut db, &v);
        for f in ["A/One.md", "A/B/Two.md", "A/B/C/Three.md", "A/B/C/D/Four.md"] {
            assert!(dir.join(f).exists(), "{f}");
        }
        // Renaming B renames its directory, with everything below.
        lifecycle::edit_folder(&mut db, b.id, &FolderEdit { title: Some("Bee".into()), ..FolderEdit::default() }).unwrap();
        let known = v.known(&db).unwrap();
        v.flush(&mut db, &known).unwrap();
        assert!(dir.join("A/Bee/C/D/Four.md").exists() && !dir.join("A/B").exists());
        // Moving C to the top takes D along.
        lifecycle::move_folder_into(&mut db, c.id, None).unwrap();
        let known = v.known(&db).unwrap();
        v.flush(&mut db, &known).unwrap();
        assert!(dir.join("C/D/Four.md").exists() && dir.join("C/Three.md").exists());
        assert!(!dir.join("A/Bee/C").exists());
        // Nothing is left to do, and reading it changes nothing.
        assert!(!sync_flush(&mut db, &v).changed());
        assert_eq!(titles(&db), ["Four", "One", "Three", "Two"]);
    }

    #[test]
    fn a_directory_moved_outside_brings_its_notes_along_as_a_new_folder() {
        let (mut db, v, dir) = setup();
        let a = lifecycle::create_folder(&mut db, "A", false).unwrap();
        let b = lifecycle::create_folder_in(&mut db, Some(a.id), "B", false).unwrap();
        let n = lifecycle::create(&mut db, &NewNote { title: "Deep".into(), folder: Some(b.id), ..NewNote::default() }).unwrap();
        sync_flush(&mut db, &v);
        fs::create_dir_all(dir.join("Elsewhere")).unwrap();
        fs::rename(dir.join("A/B"), dir.join("Elsewhere/B")).unwrap();
        let r = sync_flush(&mut db, &v);
        assert_eq!(r.moved, 1, "{r:?}");
        let now = Note::from(&db.get(n.id).unwrap());
        let path = lifecycle::folder_path(&db, now.folder_id.unwrap()).unwrap();
        let names: Vec<&str> = path.iter().map(|f| f.title.as_str()).collect();
        assert_eq!(names, ["Elsewhere", "B"]);
        assert_eq!(titles(&db), ["Deep"], "no copy");
    }

    #[test]
    fn two_notes_with_one_title_get_numbered_files() {
        let (mut db, v, dir) = setup();
        for _ in 0..3 {
            lifecycle::create(&mut db, &NewNote::new("Same")).unwrap();
        }
        lifecycle::create(&mut db, &NewNote::new("Odd: name?")).unwrap();
        sync_flush(&mut db, &v);
        for name in ["Same.md", "Same-2.md", "Same-3.md", "Odd- name-.md"] {
            assert!(dir.join(name).exists(), "{name}");
        }
        // Their names are stable.
        assert!(!sync_flush(&mut db, &v).changed());
    }

    #[test]
    fn files_claiming_one_id_are_resolved_by_path_then_age_and_the_rest_are_new_notes() {
        let (mut db, v, dir) = setup();
        let n = lifecycle::create(&mut db, &NewNote { title: "Orig".into(), body: "x".into(), ..NewNote::default() }).unwrap();
        sync_flush(&mut db, &v);
        let text = read(&dir, "Orig.md");
        fs::write(dir.join("Zcopy.md"), &text).unwrap();
        fs::write(dir.join("Acopy.md"), &text).unwrap();
        let r = v.sync(&mut db).unwrap();
        assert_eq!(r.created, 2, "{r:?}");
        // The alphabetically first path ("Acopy.md") kept the note; "Orig.md" and "Zcopy.md" are new.
        let kept = Note::from(&db.get(n.id).unwrap());
        assert_eq!(db.get(n.id).unwrap().fields["file"], "Acopy.md");
        assert_eq!(kept.title, "Orig");
        assert_eq!(titles(&db).len(), 3);
        // Three notes called Orig: the names are numbered.
        let known = v.known(&db).unwrap();
        v.flush(&mut db, &known).unwrap();
        for f in ["Orig.md", "Orig-2.md", "Orig-3.md"] {
            assert!(dir.join(f).exists(), "{f}");
        }
        assert!(!dir.join("Acopy.md").exists() && !dir.join("Zcopy.md").exists());
        // Every file now has an id of its own, and they stay settled.
        let ids: HashSet<String> = ["Orig.md", "Orig-2.md", "Orig-3.md"]
            .iter()
            .map(|f| read(&dir, f).lines().nth(1).unwrap().to_string())
            .collect();
        assert_eq!(ids.len(), 3);
        assert!(!sync_flush(&mut db, &v).changed());
    }

    #[test]
    fn when_both_changed_the_file_wins_and_the_note_is_kept_beside_it() {
        let (mut db, v, dir) = setup();
        let n = lifecycle::create(&mut db, &NewNote { title: "Both".into(), body: "start".into(), ..NewNote::default() }).unwrap();
        sync_flush(&mut db, &v);
        // Changed in the app, not yet written...
        lifecycle::edit(&mut db, n.id, &Edit { body: Some("app".into()), ..Edit::default() }).unwrap();
        // ...and in the file.
        let text = read(&dir, "Both.md").replace("start", "file");
        fs::write(dir.join("Both.md"), text).unwrap();
        let r = v.sync(&mut db).unwrap();
        assert_eq!(r.conflicts, 1, "{r:?}");
        assert_eq!(Note::from(&db.get(n.id).unwrap()).body, "file");
        let copy = read(&dir, "Both (conflict).md");
        assert!(copy.contains("app") && !copy.contains("id:"), "{copy}");
        // The copy is a note of its own the next time.
        sync_flush(&mut db, &v);
        assert_eq!(titles(&db), ["Both", "Both"]);
    }

    #[test]
    fn a_plain_file_that_is_already_a_note_is_not_duplicated() {
        let (mut db, v, dir) = setup();
        let f = lifecycle::create_folder(&mut db, "Docs", false).unwrap();
        lifecycle::create(&mut db, &NewNote { title: "Known".into(), body: "b".into(), folder: Some(f.id), ..NewNote::default() }).unwrap();
        fs::create_dir_all(dir.join("Docs")).unwrap();
        fs::write(dir.join("Docs/Known.md"), "# Known\n\nb\n").unwrap();
        let r = v.sync(&mut db).unwrap();
        assert_eq!(r.created, 0, "{r:?}");
        assert_eq!(titles(&db), ["Known"]);
    }

    #[test]
    fn adding_a_file_moves_it_in_and_a_link_is_the_fallback() {
        let (mut db, v, dir) = setup();
        let outside = temp();
        let src = outside.join("Draft.md");
        fs::write(&src, "# Draft\n\nwords\n").unwrap();
        let (dest, linked) = v.add_file(&db, &src, None).unwrap();
        assert!(!linked && dest.exists() && !src.exists(), "moved");
        // A link in the vault leads to the original, which is written through.
        let src2 = outside.join("Linked.md");
        fs::write(&src2, "# Linked\n\nold\n").unwrap();
        let at = dir.join("Linked.md");
        link(&src2, &at).unwrap();
        sync_flush(&mut db, &v);
        assert_eq!(titles(&db), ["Draft", "Linked"]);
        let id = db.list(&Query { kind: Some(NOTE.into()), ..Query::default() }).unwrap().iter().find(|i| i.title == "Linked").unwrap().id;
        lifecycle::edit(&mut db, id, &Edit { body: Some("new".into()), ..Edit::default() }).unwrap();
        sync_flush(&mut db, &v);
        assert!(is_symlink(&at), "still a link");
        assert!(fs::read_to_string(&src2).unwrap().contains("new"), "written through");
        // Deleting the note for good removes the link, never the original.
        lifecycle::trash(&mut db, id, 1).unwrap();
        sync_flush(&mut db, &v);
        let before = v.known(&db).unwrap();
        lifecycle::purge(&mut db, id).unwrap();
        v.flush(&mut db, &before).unwrap();
        assert!(src2.exists());
        let _ = fs::remove_dir_all(outside);
    }

    #[test]
    fn a_dead_link_is_reported_and_left_alone_and_linked_directories_are_not_followed() {
        let (mut db, v, dir) = setup();
        let outside = temp();
        link(&outside.join("nope.md"), &dir.join("Dead.md")).unwrap();
        fs::write(outside.join("Inside.md"), "# Inside\n").unwrap();
        link(&outside, &dir.join("Linked")).unwrap();
        let r = v.sync(&mut db).unwrap();
        assert!(r.warnings.iter().any(|w| w.contains("Dead.md")), "{r:?}");
        assert_eq!(titles(&db).len(), 0, "nothing was read through the linked directory");
        assert!(is_symlink(&dir.join("Dead.md")));
        let _ = fs::remove_dir_all(outside);
    }

    #[test]
    fn folder_settings_travel_in_the_toml_file() {
        let (mut db, v, dir) = setup();
        let f = lifecycle::create_folder(&mut db, "Work", true).unwrap();
        lifecycle::edit_folder(&mut db, f.id, &FolderEdit { notes: Some("About \"work\"".into()), color: Some(Color::parse("#112233")), ..FolderEdit::default() }).unwrap();
        sync_flush(&mut db, &v);
        let toml = read(&dir, "Work/.folder.toml");
        assert!(toml.contains("rated = true") && toml.contains("color = \"#112233\""), "{toml}");
        // Changed outside, it is read back.
        fs::write(dir.join("Work/.folder.toml"), toml.replace("#112233", "#445566")).unwrap();
        v.sync(&mut db).unwrap();
        assert_eq!(Folder::from(&db.get(f.id).unwrap()).color, Color::parse("#445566"));
        assert_eq!(Folder::from(&db.get(f.id).unwrap()).notes, "About \"work\"");
    }

    #[test]
    fn a_vault_may_not_hold_the_database() {
        let dir = temp();
        let db = dir.join("notes.db");
        fs::write(&db, "").unwrap();
        assert!(Vault::open(&dir, Some(&db)).is_err());
        assert!(Vault::open(&dir, Some(Path::new("/nowhere/else.db"))).is_ok());
    }

    #[test]
    fn names_are_made_safe() {
        assert_eq!(sanitise("a/b\\c:d"), "a-b-c-d");
        assert_eq!(sanitise("..hidden"), "hidden");
        assert_eq!(sanitise("   "), "Untitled");
        assert_eq!(sanitise(&"x".repeat(300)).chars().count(), 100);
    }
}
