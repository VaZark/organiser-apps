//! The vault: the tasks as Markdown files in a directory the person chooses. The files are the
//! tasks; the database is an index of them that keeps what a file cannot (the ids Sticky links to,
//! the order of things, the sticky itself).
//!
//! ```text
//! <vault>/Buy milk.md                   a task with no subtasks: just a file
//! <vault>/Holiday/                      an epic: a directory with no index.md
//! <vault>/Holiday/.epic.toml            what a directory cannot say: dates, colour, icon, sections
//! <vault>/Holiday/Pack.md               a task of the epic
//! <vault>/Holiday/Book flights/         a task with subtasks: a directory with index.md
//! <vault>/Holiday/Book flights/index.md   the task itself
//! <vault>/Holiday/Book flights/Prices.md  a subtask (and a directory in it is one with subtasks)
//! <vault>/.archive/...                  archived tasks, the same shape
//! ```
//!
//! Two directions, each run when the app starts, when its window is focused and on request:
//!
//! * [`Vault::sync`] reads the files: new ones become tasks, changed ones update their task, moved
//!   ones move it (and its subtasks), missing ones archive it.
//! * [`Vault::flush`] writes the tasks: every change made in the app reaches its file, a task
//!   that gains its first subtask becomes `Name/index.md` and one that loses its last goes back to
//!   `Name.md`, and a task deleted for good loses its file.
//!
//! Sync always runs first, so what was changed outside is never overwritten. If a file and its
//! task were both changed since they last agreed, the file wins and the task's version is kept
//! beside it as `<name> (conflict).md`.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use organiser_engine::{Db, Error, Item, ItemPatch, Query, Result};
use organiser_vault::{
    MAX_DEPTH, MAX_FILE, hash, io, is_markdown, is_symlink, link, prune, read_dir, unique,
    write_atomic,
};
pub use organiser_vault::{Report, sanitise};
use serde_json::Value;

use super::lifecycle::{self, NewTask};
use crate::primitives::md_task::{FileTask, parse_file, render_file};
use crate::primitives::status::Statuses;
use crate::primitives::task::{Edit, Epic, Task};
use crate::{EPIC, SECTION, TASK, statuses};

/// The directory of archived tasks.
pub const ARCHIVE: &str = ".archive";
const EPIC_META: &str = ".epic.toml";
const INDEX: &str = "index.md";

/// A directory of tasks.
#[derive(Debug, Clone)]
pub struct Vault {
    dir: PathBuf,
}

fn field<'a>(item: &'a Item, key: &str) -> Option<&'a str> {
    item.fields.get(key).and_then(Value::as_str).filter(|s| !s.is_empty())
}

/// The directory of every epic, relative to the vault, from their titles.
struct Layout {
    dir_of: HashMap<i64, String>,
    by_dir: HashMap<String, i64>,
}

impl Layout {
    fn load(db: &Db) -> Result<Self> {
        let mut epics: Vec<Epic> = db
            .list(&Query {
                kind: Some(EPIC.into()),
                ..Query::default()
            })?
            .iter()
            .map(Epic::from)
            .collect();
        epics.sort_by_key(|e| e.id);
        let mut l = Layout {
            dir_of: HashMap::new(),
            by_dir: HashMap::new(),
        };
        let mut used = HashSet::new();
        for e in epics {
            let d = unique(&sanitise(&e.title), &mut used);
            l.dir_of.insert(e.id, d.clone());
            l.by_dir.insert(d, e.id);
        }
        Ok(l)
    }
}

/// What the names in a file stand for: the person's statuses, and the sections by id.
struct Names {
    statuses: Statuses,
    sections: HashMap<i64, String>,
}

impl Names {
    fn load(db: &Db) -> Result<Self> {
        let sections = db
            .list(&Query {
                kind: Some(SECTION.into()),
                ..Query::default()
            })?
            .into_iter()
            .map(|i| (i.id, i.title))
            .collect();
        Ok(Self {
            statuses: statuses::load(db)?,
            sections,
        })
    }

    fn status(&self, t: &Task) -> Option<String> {
        self.statuses.resolve(t.status).map(|s| s.name.clone())
    }

    fn section(&self, t: &Task) -> Option<String> {
        t.section.and_then(|s| self.sections.get(&s)).cloned()
    }
}

/// A file found in the vault.
struct Found {
    /// The task's file relative to the vault, with `/`.
    rel: String,
    hash: String,
    task: FileTask,
    /// The directory of the epic it is in, if any.
    epic_dir: Option<String>,
    /// The task whose directory it is in (an index into the list found).
    parent: Option<usize>,
    archived: bool,
    /// The `id` it named, when another file has it: this one is a task of its own.
    lost: Option<i64>,
}

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
                "{} holds the database; pick another folder for the tasks",
                dir.display()
            )));
        }
        Ok(Self { dir })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The file of a task, if it has one.
    pub fn file_of(&self, db: &Db, id: i64) -> Option<PathBuf> {
        let item = db.get(id).ok()?;
        field(&item, "file").map(|f| self.dir.join(f))
    }

    // ---- reading the files into the tasks ---------------------------------------------------

    /// Bring the tasks up to date with the files.
    pub fn sync(&self, db: &mut Db) -> Result<Report> {
        let mut rep = Report::default();
        statuses::ensure_defaults(db)?;
        let (mut found, epic_dirs) = self.scan(&mut rep);

        // An epic is a directory: made when it is new; its `.epic.toml` says the rest.
        let mut layout = Layout::load(db)?;
        for name in &epic_dirs {
            let meta = self.dir.join(name).join(EPIC_META);
            let id = match layout.by_dir.get(name) {
                Some(id) => *id,
                None => {
                    let title = epic_title(&meta).unwrap_or_else(|| name.clone());
                    lifecycle::create_epic(db, &title)?.id
                }
            };
            self.read_epic_meta(db, id, &meta)?;
            layout = Layout::load(db)?;
        }
        // (An archived epic directory is the same epic: they share names.)

        // Two files claiming one task: the one first by path keeps it, then the older; the others
        // are tasks of their own.
        let mut claims: HashMap<i64, Vec<usize>> = HashMap::new();
        for (i, f) in found.iter().enumerate() {
            if let Some(id) = f.task.id {
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
                    .then(fa.task.created.unwrap_or(i64::MAX).cmp(&fb.task.created.unwrap_or(i64::MAX)))
            });
            for loser in &idx[1..] {
                found[*loser].lost = found[*loser].task.id.take();
            }
        }

        let names = Names::load(db)?;
        let items = self.task_items(db)?;
        let by_id: HashMap<i64, &Item> = items.iter().map(|i| (i.id, i)).collect();
        let by_file: HashMap<String, i64> = items
            .iter()
            .filter_map(|i| field(i, "file").map(|f| (f.to_string(), i.id)))
            .collect();
        // Tasks not yet tied to a file, to adopt a plain file that is the same task.
        let mut untied: HashMap<(Option<i64>, String), i64> = HashMap::new();
        for i in &items {
            if field(i, "file").is_none() {
                let t = Task::from(i);
                untied.insert((t.epic, t.title.clone()), t.id);
            }
        }

        let mut seen: HashSet<i64> = HashSet::new();
        let mut resolved: Vec<i64> = Vec::new();
        for f in &found {
            let epic = f.epic_dir.as_ref().and_then(|d| layout.by_dir.get(d)).copied();
            let parent = f.parent.map(|p| resolved[p]);
            let task_id = f
                .task
                .id
                .filter(|id| by_id.contains_key(id) && !seen.contains(id))
                .or_else(|| {
                    let named = f.task.id.or(f.lost);
                    if named.is_some_and(|id| by_id.contains_key(&id)) {
                        return None;
                    }
                    by_file
                        .get(&f.rel)
                        .copied()
                        .filter(|id| !seen.contains(id))
                        .or_else(|| untied.remove(&(epic, f.task.title.clone())))
                });
            let id = match task_id {
                Some(id) => {
                    seen.insert(id);
                    let item = db.get(id)?;
                    self.read_into(db, &item, f, epic, parent, &names, &mut rep)?;
                    id
                }
                None => {
                    let id = self.create_from(db, f, epic, parent, &names)?;
                    seen.insert(id);
                    rep.created += 1;
                    id
                }
            };
            resolved.push(id);
        }

        // A task whose file is gone is archived; if it already was, it goes for good.
        for i in &items {
            if seen.contains(&i.id) || field(i, "file").is_none() {
                continue;
            }
            if i.archived {
                db.delete(i.id).or_else(|e| match e {
                    Error::NotFound(_) => Ok(0),
                    e => Err(e),
                })?;
            } else {
                lifecycle::set_archived(db, i.id, true)?;
                rep.trashed += 1;
            }
        }
        Ok(rep)
    }

    fn task_items(&self, db: &Db) -> Result<Vec<Item>> {
        db.list(&Query {
            kind: Some(TASK.into()),
            ..Query::default()
        })
    }

    /// The section of an epic by name, made when it is not there.
    fn section_named(&self, db: &mut Db, epic: i64, name: &str) -> Result<i64> {
        for c in db.children(Some(epic))? {
            if c.kind == SECTION && c.title.eq_ignore_ascii_case(name) {
                return Ok(c.id);
            }
        }
        Ok(lifecycle::create_section(db, epic, name)?.id)
    }

    fn create_from(
        &self,
        db: &mut Db,
        f: &Found,
        epic: Option<i64>,
        parent: Option<i64>,
        names: &Names,
    ) -> Result<i64> {
        // A subtask is in its parent's section; any other, in the one the file names.
        let section = match (parent, epic, &f.task.section) {
            (Some(p), _, _) => Task::from(&db.get(p)?).section,
            (None, Some(e), Some(name)) => Some(self.section_named(db, e, name)?),
            _ => None,
        };
        let status = f
            .task
            .status
            .as_deref()
            .and_then(|n| names.statuses.by_name(n))
            .map(|s| s.id);
        let t = lifecycle::create(
            db,
            &NewTask {
                title: f.task.title.clone(),
                notes: f.task.notes.clone(),
                parent,
                epic,
                section,
                start: f.task.start,
                due: f.task.due,
                priority: f.task.priority,
                status,
                tags: f.task.tags.clone(),
                pinned: f.task.pinned,
            },
        )?;
        if f.archived || f.task.archived {
            lifecycle::set_archived(db, t.id, true)?;
        }
        self.tie(db, t.id, f, false)?;
        Ok(t.id)
    }

    /// Record that the task is this file as it is now. `again` asks for the file to be written
    /// once more by the next flush.
    fn tie(&self, db: &mut Db, id: i64, f: &Found, again: bool) -> Result<()> {
        let task = Task::from(&db.get(id)?);
        let names = Names::load(db)?;
        let layout = Layout::load(db)?;
        let sig = if again || f.task.id != Some(id) {
            String::new()
        } else {
            sig(&names, &layout, &task)
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

    /// Make an existing task match its file, if the file has changed (or moved).
    #[allow(clippy::too_many_arguments)]
    fn read_into(
        &self,
        db: &mut Db,
        item: &Item,
        f: &Found,
        epic: Option<i64>,
        parent: Option<i64>,
        names: &Names,
        rep: &mut Report,
    ) -> Result<()> {
        let task = Task::from(item);
        let layout = Layout::load(db)?;
        let stored = field(item, "file_hash").unwrap_or("");
        let row_changed = field(item, "file_sig").is_some_and(|s| s != sig(names, &layout, &task));
        let file_changed = stored != f.hash;
        let archived = f.archived || f.task.archived;
        let placed = task.parent_id == parent
            && task.epic == epic
            && task.archived == archived
            && field(item, "file") == Some(f.rel.as_str());
        if !file_changed && placed {
            return Ok(());
        }
        if !file_changed && row_changed {
            return Ok(());
        }
        if file_changed && row_changed && field(item, "file_sig").is_some() {
            self.keep_conflict(&task, names, &layout, &f.rel, rep)?;
            rep.conflicts += 1;
        }
        let mut changed = self.apply(db, &task, f, epic, parent, names)?;
        if !file_changed && !placed {
            rep.moved += usize::from(changed);
            changed = false;
        }
        if changed {
            rep.updated += 1;
        }
        self.tie(db, task.id, f, false)?;
        Ok(())
    }

    /// Put the file's content into the task. True if anything differed.
    fn apply(
        &self,
        db: &mut Db,
        cur: &Task,
        f: &Found,
        epic: Option<i64>,
        parent: Option<i64>,
        names: &Names,
    ) -> Result<bool> {
        let id = cur.id;
        let mut changed = false;
        let want = &f.task;
        // Whose subtask it is (the path says).
        if cur.parent_id != parent && parent != Some(id) {
            db.move_item(id, parent, None)?;
            changed = true;
        }
        let mut edit = Edit {
            title: (cur.title != want.title).then(|| want.title.clone()),
            notes: (cur.notes != want.notes).then(|| want.notes.clone()),
            start: (cur.start != want.start).then_some(want.start),
            due: (cur.due != want.due).then_some(want.due),
            priority: want.priority.filter(|p| *p != cur.priority),
            ..Edit::default()
        };
        // Its epic, and its section: a subtask is in its parent's, any other in the one named.
        let want_section = match (parent, epic, &want.section) {
            (Some(p), _, _) => Task::from(&db.get(p)?).section,
            (None, Some(e), Some(name)) => Some(self.section_named(db, e, name)?),
            _ => None,
        };
        if cur.epic != epic || cur.section != want_section {
            edit.epic = Some(epic);
            edit.section = Some(want_section);
        }
        if edit.title.is_some()
            || edit.notes.is_some()
            || edit.start.is_some()
            || edit.due.is_some()
            || edit.priority.is_some()
            || edit.epic.is_some()
        {
            lifecycle::edit(db, id, &edit)?;
            changed = true;
        }
        if cur.tags != want.tags {
            lifecycle::set_tags(db, id, &want.tags)?;
            changed = true;
        }
        if cur.pinned != want.pinned {
            lifecycle::set_pinned(db, id, want.pinned)?;
            changed = true;
        }
        // The status is named in the file; an unknown name leaves it alone.
        if let Some(s) = want.status.as_deref().and_then(|n| names.statuses.by_name(n))
            && names.statuses.resolve(cur.status).map(|r| r.id) != Some(s.id)
        {
            db.update(id, ItemPatch::default().field("status", s.id))?;
            changed = true;
        }
        let archived = f.archived || want.archived;
        if cur.archived != archived {
            lifecycle::set_archived(db, id, archived)?;
            changed = true;
        }
        Ok(changed)
    }

    /// Save the task's own version next to the file that won, as a task without an id.
    fn keep_conflict(&self, task: &Task, names: &Names, layout: &Layout, rel: &str, rep: &mut Report) -> Result<()> {
        let _ = layout;
        let path = self.dir.join(rel);
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("task").to_string();
        // An `index.md` is named for its directory.
        let (dir, stem) = if stem == "index" {
            let d = path.parent().unwrap_or(&self.dir);
            (
                d.parent().unwrap_or(&self.dir).to_path_buf(),
                d.file_name().and_then(|n| n.to_str()).unwrap_or("task").to_string(),
            )
        } else {
            (path.parent().unwrap_or(&self.dir).to_path_buf(), stem)
        };
        let mut copy = dir.join(format!("{stem} (conflict).md"));
        let mut n = 1;
        while copy.exists() {
            n += 1;
            copy = dir.join(format!("{stem} (conflict {n}).md"));
        }
        let text = render_file(task, names.status(task).as_deref(), names.section(task).as_deref(), None)
            .replacen(&format!("id: {}\n", task.id), "", 1);
        write_atomic(&copy, &text)?;
        rep.warnings.push(format!(
            "{rel} was changed in the app and outside it; the file was kept, the app's version is {}",
            copy.file_name().and_then(|n| n.to_str()).unwrap_or("beside it")
        ));
        Ok(())
    }

    /// Every task file in the vault (a task's own file before the ones in its directory), and the
    /// directories of the epics.
    fn scan(&self, rep: &mut Report) -> (Vec<Found>, Vec<String>) {
        let mut found = Vec::new();
        let mut epics: Vec<String> = Vec::new();
        self.scan_base(&self.dir, false, &mut found, &mut epics, rep);
        self.scan_base(&self.dir.join(ARCHIVE), true, &mut found, &mut epics, rep);
        epics.sort();
        epics.dedup();
        (found, epics)
    }

    /// The top of the vault (or of its archive): the files in it are tasks of no epic, a directory
    /// with an `index.md` is a task, and any other is an epic.
    fn scan_base(&self, base: &Path, archived: bool, out: &mut Vec<Found>, epics: &mut Vec<String>, rep: &mut Report) {
        self.scan_files(base, None, None, archived, out, rep);
        for name in subdirs(base) {
            let path = base.join(&name);
            if path.join(INDEX).is_file() {
                self.scan_task_dir(&path, None, None, archived, 1, out, rep);
            } else {
                epics.push(name.clone());
                self.scan_files(&path, Some(&name), None, archived, out, rep);
                for sub in subdirs(&path) {
                    let p = path.join(&sub);
                    if p.join(INDEX).is_file() {
                        self.scan_task_dir(&p, Some(&name), None, archived, 1, out, rep);
                    }
                }
            }
        }
    }

    /// A task's directory: `index.md` is the task; the other files in it, and the directories that
    /// have an `index.md`, are its subtasks.
    #[allow(clippy::too_many_arguments)]
    fn scan_task_dir(
        &self,
        path: &Path,
        epic: Option<&str>,
        parent: Option<usize>,
        archived: bool,
        depth: usize,
        out: &mut Vec<Found>,
        rep: &mut Report,
    ) {
        let before = out.len();
        self.read_one(&path.join(INDEX), epic, parent, archived, path.file_name().and_then(|n| n.to_str()), out, rep);
        if out.len() == before || depth >= MAX_DEPTH {
            return;
        }
        let me = Some(before);
        self.scan_files_except(path, epic, me, archived, INDEX, out, rep);
        for sub in subdirs(path) {
            let p = path.join(&sub);
            if p.join(INDEX).is_file() {
                self.scan_task_dir(&p, epic, me, archived, depth + 1, out, rep);
            }
        }
    }

    fn scan_files(&self, path: &Path, epic: Option<&str>, parent: Option<usize>, archived: bool, out: &mut Vec<Found>, rep: &mut Report) {
        self.scan_files_except(path, epic, parent, archived, "", out, rep);
    }

    #[allow(clippy::too_many_arguments)]
    fn scan_files_except(
        &self,
        path: &Path,
        epic: Option<&str>,
        parent: Option<usize>,
        archived: bool,
        skip: &str,
        out: &mut Vec<Found>,
        rep: &mut Report,
    ) {
        let mut files: Vec<_> = read_dir(path)
            .into_iter()
            .filter(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                is_markdown(&e.path()) && !n.starts_with('.') && n != skip
            })
            .collect();
        files.sort_by_key(|e| e.file_name());
        for e in files {
            let p = e.path();
            let stem = p.file_stem().and_then(|s| s.to_str()).map(String::from);
            self.read_one(&p, epic, parent, archived, stem.as_deref(), out, rep);
        }
    }

    /// Read one task file into the list, if it can be read.
    #[allow(clippy::too_many_arguments)]
    fn read_one(
        &self,
        p: &Path,
        epic: Option<&str>,
        parent: Option<usize>,
        archived: bool,
        fallback: Option<&str>,
        out: &mut Vec<Found>,
        rep: &mut Report,
    ) {
        let rel = p.strip_prefix(&self.dir).unwrap_or(p).to_string_lossy().replace('\\', "/");
        // A link to a file is followed; one that leads nowhere is reported.
        let meta = match fs::metadata(p) {
            Ok(m) if m.is_file() => m,
            Ok(_) => return,
            Err(_) => {
                rep.warnings.push(format!("{rel} is a link to a file that is not there"));
                return;
            }
        };
        if meta.len() > MAX_FILE {
            rep.warnings.push(format!("{rel} is too big to be a task"));
            return;
        }
        let Ok(text) = fs::read_to_string(p) else {
            rep.warnings.push(format!("{rel} is not text"));
            return;
        };
        out.push(Found {
            hash: hash(&text),
            task: parse_file(&text, fallback.unwrap_or("Untitled")),
            rel,
            epic_dir: epic.map(String::from),
            parent,
            archived,
            lost: None,
        });
    }

    fn read_epic_meta(&self, db: &mut Db, id: i64, path: &Path) -> Result<()> {
        let Ok(text) = fs::read_to_string(path) else {
            return Ok(());
        };
        let Ok(table) = text.parse::<toml::Table>() else {
            return Ok(());
        };
        let cur = Epic::from(&db.get(id)?);
        let get = |k: &str| table.get(k).and_then(toml::Value::as_str).map(String::from);
        let mut edit = Edit::default();
        if let Some(t) = get("title").filter(|t| *t != cur.title) {
            edit.title = Some(t);
        }
        let notes = get("description").unwrap_or_default();
        if notes != cur.notes {
            edit.notes = Some(notes);
        }
        let color = get("color");
        if color != cur.color {
            edit.color = Some(color);
        }
        let icon = get("icon");
        if icon != cur.icon {
            edit.icon = Some(icon);
        }
        let start = get("start").and_then(|d| d.parse().ok());
        if start != cur.start {
            edit.start = Some(start);
        }
        let due = get("due").and_then(|d| d.parse().ok());
        if due != cur.due {
            edit.due = Some(due);
        }
        if edit.title.is_some()
            || edit.notes.is_some()
            || edit.color.is_some()
            || edit.icon.is_some()
            || edit.start.is_some()
            || edit.due.is_some()
        {
            lifecycle::edit_epic(db, id, &edit)?;
        }
        // Its sections, in the order the file lists them.
        if let Some(list) = table.get("section").and_then(toml::Value::as_array) {
            for (at, s) in list.iter().filter_map(toml::Value::as_table).enumerate() {
                let Some(title) = s.get("title").and_then(toml::Value::as_str) else { continue };
                let sid = self.section_named(db, id, title)?;
                let sget = |k: &str| s.get(k).and_then(toml::Value::as_str).map(String::from);
                let cur = crate::primitives::task::Section::from(&db.get(sid)?);
                let mut e = Edit::default();
                let notes = sget("description").unwrap_or_default();
                if notes != cur.notes {
                    e.notes = Some(notes);
                }
                let color = sget("color");
                if color != cur.color {
                    e.color = Some(color);
                }
                if e.notes.is_some() || e.color.is_some() {
                    lifecycle::edit_section(db, sid, &e)?;
                }
                lifecycle::move_section(db, sid, at)?;
            }
        }
        Ok(())
    }

    // ---- writing the tasks out ---------------------------------------------------------------

    /// The file each task has now, to hand back to [`flush`](Self::flush): that is how a task
    /// deleted for good is known to have had one.
    pub fn known(&self, db: &Db) -> Result<BTreeMap<i64, String>> {
        Ok(self
            .task_items(db)?
            .iter()
            .filter_map(|i| field(i, "file").map(|f| (i.id, f.to_string())))
            .collect())
    }

    /// Bring the files up to date with the tasks. `before` is [`known`](Self::known) from before
    /// the changes: a task in it that is gone loses its file.
    pub fn flush(&self, db: &mut Db, before: &BTreeMap<i64, String>) -> Result<Report> {
        let mut rep = Report::default();
        let layout = Layout::load(db)?;
        let names = Names::load(db)?;
        let items = self.task_items(db)?;
        let live: HashSet<i64> = items.iter().map(|i| i.id).collect();

        // A task deleted for good: its file, if it is still that task's.
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

        self.write_epic_meta(db, &layout)?;

        let plan = self.plan(db, &layout, &items);
        // A file whose place is held by another task's file that has yet to move waits for it: a
        // few rounds settle a swap or a shuffle, and the last bumps a name rather than wait.
        let mut pending: Vec<i64> = items.iter().map(|i| i.id).filter(|id| plan.contains_key(id)).collect();
        for round in 0..4 {
            let now_items: HashMap<i64, Item> = self.task_items(db)?.into_iter().map(|i| (i.id, i)).collect();
            let owners: HashMap<String, i64> = now_items
                .values()
                .filter_map(|i| field(i, "file").map(|f| (f.to_string(), i.id)))
                .collect();
            let mut next = Vec::new();
            for id in pending {
                let (Some(item), Some(want)) = (now_items.get(&id), plan.get(&id)) else { continue };
                match self.write_one(db, &layout, &names, item, want, &owners, round == 3, &mut rep) {
                    Ok(true) => {}
                    Ok(false) => next.push(id),
                    Err(e) => rep.warnings.push(e.to_string()),
                }
            }
            if next.is_empty() {
                break;
            }
            pending = next;
        }
        let mut keep: HashSet<PathBuf> = layout.dir_of.values().map(PathBuf::from).collect();
        let mut keep_archive: HashSet<PathBuf> = keep.clone();
        for p in plan.values() {
            if let Some(d) = &p.dir {
                let d = PathBuf::from(d);
                match d.strip_prefix(ARCHIVE) {
                    Ok(inner) => {
                        keep_archive.insert(inner.to_path_buf());
                    }
                    Err(_) => {
                        keep.insert(d);
                    }
                }
            }
        }
        prune(&self.dir, Path::new(""), 0, &keep, EPIC_META);
        prune(&self.dir.join(ARCHIVE), Path::new(""), 0, &keep_archive, EPIC_META);
        Ok(rep)
    }

    fn write_epic_meta(&self, db: &Db, layout: &Layout) -> Result<()> {
        let epics: Vec<Epic> = db
            .list(&Query {
                kind: Some(EPIC.into()),
                ..Query::default()
            })?
            .iter()
            .map(Epic::from)
            .collect();
        for e in &epics {
            let Some(dir) = layout.dir_of.get(&e.id) else { continue };
            let mut t = toml::Table::new();
            t.insert("title".into(), e.title.clone().into());
            if !e.notes.is_empty() {
                t.insert("description".into(), e.notes.clone().into());
            }
            if let Some(c) = &e.color {
                t.insert("color".into(), c.clone().into());
            }
            if let Some(i) = &e.icon {
                t.insert("icon".into(), i.clone().into());
            }
            if let Some(d) = e.start {
                t.insert("start".into(), d.to_string().into());
            }
            if let Some(d) = e.due {
                t.insert("due".into(), d.to_string().into());
            }
            let mut sections = Vec::new();
            for s in db.children(Some(e.id))?.iter().filter(|i| i.kind == SECTION) {
                let s = crate::primitives::task::Section::from(s);
                let mut st = toml::Table::new();
                st.insert("title".into(), s.title.into());
                if !s.notes.is_empty() {
                    st.insert("description".into(), s.notes.into());
                }
                if let Some(c) = s.color {
                    st.insert("color".into(), c.into());
                }
                sections.push(toml::Value::Table(st));
            }
            if !sections.is_empty() {
                t.insert("section".into(), toml::Value::Array(sections));
            }
            let text = toml::to_string(&t).unwrap_or_default();
            let path = self.dir.join(dir).join(EPIC_META);
            if fs::read_to_string(&path).is_ok_and(|old| old == text) {
                continue;
            }
            write_atomic(&path, &text)?;
        }
        Ok(())
    }

    /// Where every task's file goes: its epic's directory, then its parents' directories, and
    /// `Name.md` for a task with no subtasks or `Name/index.md` for one with.
    fn plan(&self, db: &Db, layout: &Layout, items: &[Item]) -> HashMap<i64, Place> {
        let _ = db;
        let mut kids: HashMap<Option<i64>, Vec<&Item>> = HashMap::new();
        for i in items {
            kids.entry(i.parent_id).or_default().push(i);
        }
        for v in kids.values_mut() {
            v.sort_by_key(|i| (i.position, i.id));
        }
        let mut taken: HashMap<String, HashSet<String>> = HashMap::new();
        let mut out = HashMap::new();
        for root in kids.get(&None).cloned().unwrap_or_default() {
            let t = Task::from(root);
            let epic_dir = t.epic.and_then(|e| layout.dir_of.get(&e)).cloned();
            let mut dir = String::new();
            if t.archived {
                dir.push_str(ARCHIVE);
            }
            if let Some(e) = epic_dir {
                if !dir.is_empty() {
                    dir.push('/');
                }
                dir.push_str(&e);
            }
            self.plan_task(root, &dir, &kids, &mut taken, &mut out, 0);
        }
        out
    }

    fn plan_task(
        &self,
        item: &Item,
        dir: &str,
        kids: &HashMap<Option<i64>, Vec<&Item>>,
        taken: &mut HashMap<String, HashSet<String>>,
        out: &mut HashMap<i64, Place>,
        depth: usize,
    ) {
        let children = kids.get(&Some(item.id)).cloned().unwrap_or_default();
        let base = sanitise(&item.title);
        // Keep the name it has while it is in the right place and still suits the title.
        let kept = field(item, "file").and_then(|f| {
            let p = Path::new(f);
            let (stem, parent) = if p.file_name().and_then(|n| n.to_str()) == Some(INDEX) {
                let d = p.parent()?;
                (d.file_name()?.to_str()?.to_string(), d.parent()?.to_string_lossy().replace('\\', "/"))
            } else {
                (p.file_stem()?.to_str()?.to_string(), p.parent()?.to_string_lossy().replace('\\', "/"))
            };
            let suits = stem == base
                || stem
                    .strip_prefix(&format!("{base}-"))
                    .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
            (parent == dir && suits).then_some(stem)
        });
        let set = taken.entry(dir.to_string()).or_default();
        let stem = match kept {
            Some(s) if set.insert(s.to_lowercase()) => s,
            _ => unique(&base, set),
        };
        let join = |a: &str, b: &str| if a.is_empty() { b.to_string() } else { format!("{a}/{b}") };
        let has_kids = !children.is_empty() && depth < MAX_DEPTH;
        let (rel, own_dir) = if has_kids {
            let d = join(dir, &stem);
            (join(&d, INDEX), Some(d))
        } else {
            (join(dir, &format!("{stem}.md")), None)
        };
        out.insert(
            item.id,
            Place {
                rel,
                dir: own_dir.clone().or_else(|| (!dir.is_empty()).then(|| dir.to_string())),
            },
        );
        if let Some(d) = own_dir {
            for c in children {
                self.plan_task(c, &d, kids, taken, out, depth + 1);
            }
        }
    }

    /// Write one task's file. `Ok(false)` when its place is held by another task's file that has
    /// not moved yet (and `last` is not set): try again after the others.
    #[allow(clippy::too_many_arguments)]
    fn write_one(
        &self,
        db: &mut Db,
        layout: &Layout,
        names: &Names,
        item: &Item,
        want: &Place,
        owners: &HashMap<String, i64>,
        last: bool,
        rep: &mut Report,
    ) -> Result<bool> {
        let task = Task::from(item);
        let recorded = field(item, "file").map(String::from);
        let old_path = recorded.as_ref().map(|r| self.dir.join(r));
        let on_disk = old_path.as_ref().and_then(|p| fs::read_to_string(p).ok());
        // The file was changed outside since they last agreed: the next sync takes it in.
        if let (Some(text), Some(stored)) = (&on_disk, field(item, "file_hash"))
            && hash(text) != stored
        {
            return Ok(true);
        }
        let sig = sig(names, layout, &task);
        if recorded.as_deref() == Some(want.rel.as_str())
            && field(item, "file_sig") == Some(sig.as_str())
            && on_disk.is_some()
        {
            return Ok(true);
        }
        // A name taken by a file that is not this task's (one not read yet) is not overwritten.
        let mut rel = want.rel.clone();
        let mut n = 1;
        while self.dir.join(&rel).exists() && recorded.as_deref() != Some(rel.as_str()) {
            if !last && owners.get(&rel).is_some_and(|o| *o != task.id) {
                return Ok(false);
            }
            n += 1;
            let p = Path::new(&want.rel);
            let (head, name) = if p.file_name().and_then(|x| x.to_str()) == Some(INDEX) {
                (p.parent().and_then(|d| d.parent()), p.parent().and_then(|d| d.file_name()).and_then(|x| x.to_str()).unwrap_or("task"))
            } else {
                (p.parent(), p.file_stem().and_then(|x| x.to_str()).unwrap_or("task"))
            };
            let stem = format!("{name}-{n}");
            let joined = head.map_or_else(PathBuf::new, Path::to_path_buf);
            rel = if p.file_name().and_then(|x| x.to_str()) == Some(INDEX) {
                joined.join(&stem).join(INDEX)
            } else {
                joined.join(format!("{stem}.md"))
            }
            .to_string_lossy()
            .replace('\\', "/");
        }
        let previous = on_disk.as_deref().and_then(|t| parse_file(t, "").front);
        let text = render_file(&task, names.status(&task).as_deref(), names.section(&task).as_deref(), previous.as_ref());
        let new_path = self.dir.join(&rel);
        if let Some(p) = new_path.parent() {
            fs::create_dir_all(p).map_err(|e| io(e, p))?;
        }
        // Moved or renamed: the file (or the link to it) goes first, then it is written.
        if let Some(old) = &old_path
            && old != &new_path
            && (old.exists() || is_symlink(old))
        {
            fs::rename(old, &new_path).map_err(|e| io(e, old))?;
        }
        write_atomic(&new_path, &text)?;
        db.update(
            task.id,
            ItemPatch::default()
                .field("file", rel.as_str())
                .field("file_hash", hash(&text))
                .field("file_sig", sig),
        )?;
        rep.written += 1;
        Ok(true)
    }

    // ---- bringing a file in -----------------------------------------------------------------

    /// Put the Markdown file `src` into the vault, in `epic` (no epic if none), so the app and any
    /// editor work on the one file. It is moved; if it cannot be (another file system, say) a link
    /// to it is put in the vault instead. Then call [`sync`](Self::sync). Returns the new path and
    /// whether it is a link.
    pub fn add_file(&self, db: &Db, src: &Path, epic: Option<i64>) -> Result<(PathBuf, bool)> {
        if !src.is_file() || !is_markdown(src) {
            return Err(Error::Invalid(format!("{} is not a Markdown file", src.display())));
        }
        let layout = Layout::load(db)?;
        let dir = match epic.and_then(|e| layout.dir_of.get(&e)) {
            Some(d) => self.dir.join(d),
            None => self.dir.clone(),
        };
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

/// Where a task's file goes, and the directory that holds it (or, for a task with subtasks, that
/// it is).
struct Place {
    rel: String,
    dir: Option<String>,
}

/// What a task would be written as, and where it is (its epic, whose subtask it is, archived), as
/// one hash: a change to any is a change to the task.
fn sig(names: &Names, layout: &Layout, t: &Task) -> String {
    hash(&format!(
        "{}\n{:?}\n{:?}\n{}",
        render_file(t, names.status(t).as_deref(), names.section(t).as_deref(), None),
        t.epic.and_then(|e| layout.dir_of.get(&e)),
        t.parent_id,
        t.archived
    ))
}

fn epic_title(meta: &Path) -> Option<String> {
    let text = fs::read_to_string(meta).ok()?;
    let t = text.parse::<toml::Table>().ok()?;
    t.get("title").and_then(toml::Value::as_str).map(String::from)
}

/// The directories of a directory, by name, not the dot ones.
fn subdirs(p: &Path) -> Vec<String> {
    let mut v: Vec<String> = read_dir(p)
        .into_iter()
        // A directory that is really one: a link to somewhere else is not followed.
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| !n.starts_with('.'))
        .collect();
    v.sort();
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SCHEMA;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static N: AtomicUsize = AtomicUsize::new(0);

    fn temp() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "todo-vault-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn setup() -> (Db, Vault, PathBuf) {
        let mut db = Db::open_in_memory(&SCHEMA).unwrap();
        statuses::ensure_defaults(&mut db).unwrap();
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

    fn flush(db: &mut Db, v: &Vault) -> Report {
        let known = v.known(db).unwrap();
        v.flush(db, &known).unwrap()
    }

    fn read(dir: &Path, rel: &str) -> String {
        fs::read_to_string(dir.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
    }

    fn new(db: &mut Db, title: &str) -> Task {
        lifecycle::create(db, &NewTask::new(title)).unwrap()
    }

    fn sub(db: &mut Db, parent: i64, title: &str) -> Task {
        lifecycle::create(db, &NewTask { parent: Some(parent), ..NewTask::new(title) }).unwrap()
    }

    fn titles(db: &Db) -> Vec<String> {
        let mut t: Vec<String> = db
            .list(&Query { kind: Some(TASK.into()), ..Query::default() })
            .unwrap()
            .iter()
            .map(|i| i.title.clone())
            .collect();
        t.sort();
        t
    }

    fn task_named(db: &Db, title: &str) -> Task {
        db.list(&Query { kind: Some(TASK.into()), ..Query::default() })
            .unwrap()
            .iter()
            .find(|i| i.title == title)
            .map(Task::from)
            .unwrap_or_else(|| panic!("no task {title}"))
    }

    #[test]
    fn a_task_is_a_file_in_its_epics_directory() {
        let (mut db, v, dir) = setup();
        let e = lifecycle::create_epic(&mut db, "Holiday").unwrap();
        let a = lifecycle::create(&mut db, &NewTask { epic: Some(e.id), notes: "text".into(), ..NewTask::new("Pack") }).unwrap();
        new(&mut db, "Loose");
        let r = flush(&mut db, &v);
        assert_eq!(r.written, 2, "{r:?}");
        assert!(read(&dir, "Holiday/Pack.md").contains("# Pack\n\ntext\n"));
        assert!(read(&dir, "Holiday/Pack.md").starts_with(&format!("---\nid: {}\n", a.id)));
        assert!(dir.join("Loose.md").exists(), "a task in no epic is a file at the top");
        assert!(read(&dir, "Holiday/.epic.toml").contains("title = \"Holiday\""));
        let known = v.known(&db).unwrap();
        assert_eq!(v.flush(&mut db, &known).unwrap().written, 0, "nothing changed: nothing written");
    }

    #[test]
    fn subtasks_make_their_parent_a_directory_with_an_index_and_losing_them_collapses_it() {
        let (mut db, v, dir) = setup();
        let e = lifecycle::create_epic(&mut db, "Trip").unwrap();
        let p = lifecycle::create(&mut db, &NewTask { epic: Some(e.id), ..NewTask::new("Book flights") }).unwrap();
        flush(&mut db, &v);
        assert!(dir.join("Trip/Book flights.md").exists());
        let c1 = lifecycle::create(&mut db, &NewTask { parent: Some(p.id), epic: Some(e.id), ..NewTask::new("Prices") }).unwrap();
        let hotel = lifecycle::create(&mut db, &NewTask { parent: Some(p.id), epic: Some(e.id), ..NewTask::new("Hotel") }).unwrap();
        lifecycle::create(&mut db, &NewTask { parent: Some(hotel.id), epic: Some(e.id), ..NewTask::new("Reviews") }).unwrap();
        flush(&mut db, &v);
        for f in ["Trip/Book flights/index.md", "Trip/Book flights/Prices.md", "Trip/Book flights/Hotel/index.md", "Trip/Book flights/Hotel/Reviews.md"] {
            assert!(dir.join(f).exists(), "{f}");
        }
        assert!(!dir.join("Trip/Book flights.md").exists(), "it moved to index.md");
        assert!(read(&dir, "Trip/Book flights/index.md").contains("# Book flights"));
        // It is the same task, and reading the tree back changes nothing.
        assert!(!sync_flush(&mut db, &v).changed());
        // Losing the subtasks collapses it back to a file.
        let before = v.known(&db).unwrap();
        db.delete(c1.id).unwrap();
        db.delete(hotel.id).unwrap();
        v.flush(&mut db, &before).unwrap();
        assert!(dir.join("Trip/Book flights.md").exists());
        assert!(!dir.join("Trip/Book flights").exists(), "the empty directory went");
        assert_eq!(Task::from(&db.get(p.id).unwrap()).title, "Book flights");
    }

    #[test]
    fn new_files_become_tasks_with_their_epic_and_parent_from_the_path() {
        let (mut db, v, dir) = setup();
        fs::create_dir_all(dir.join("Work/Report/Draft")).unwrap();
        fs::create_dir_all(dir.join("Work/Notes dir")).unwrap();
        fs::write(dir.join("Top.md"), "# Top\n\nhello\n").unwrap();
        fs::write(dir.join("Work/Plain.md"), "no heading here\n").unwrap();
        fs::write(dir.join("Work/Report/index.md"), "---\nstatus: In progress\npriority: high\ndue: 2026-12-01\ntags: [a, b]\n---\n# Report\n").unwrap();
        fs::write(dir.join("Work/Report/Intro.md"), "# Intro\n").unwrap();
        fs::write(dir.join("Work/Report/Draft/index.md"), "# Draft\n").unwrap();
        fs::write(dir.join("Work/Report/Draft/Outline.md"), "# Outline\n").unwrap();
        fs::write(dir.join("Work/Notes dir/Ignored.md"), "# Ignored\n").unwrap();
        fs::write(dir.join("Work/readme.txt"), "not a task").unwrap();
        let r = sync_flush(&mut db, &v);
        assert_eq!(r.created, 6, "{r:?}");
        assert_eq!(titles(&db), ["Draft", "Intro", "Outline", "Plain", "Report", "Top"]);
        let work = lifecycle::epic_by_title(&db, "Work").unwrap();
        let report = task_named(&db, "Report");
        assert_eq!((report.epic, report.parent_id), (Some(work), None));
        assert_eq!(report.priority, crate::primitives::priority::Priority::High);
        assert_eq!(report.due.map(|d| d.to_string()).as_deref(), Some("2026-12-01"));
        assert_eq!(report.tags, ["a", "b"]);
        let st = statuses::load(&db).unwrap();
        assert_eq!(st.resolve(report.status).unwrap().name, "In progress");
        for (child, parent) in [("Intro", "Report"), ("Draft", "Report"), ("Outline", "Draft")] {
            let c = task_named(&db, child);
            assert_eq!(c.parent_id, Some(task_named(&db, parent).id), "{child}");
            assert_eq!(c.epic, Some(work), "{child} is in the epic too");
        }
        assert_eq!(task_named(&db, "Top").epic, None);
        // Files that had no id got one when written back, and a second look changes nothing.
        assert!(read(&dir, "Top.md").contains("id: "));
        assert!(!sync_flush(&mut db, &v).changed());
        assert_eq!(titles(&db).len(), 6);
    }

    #[test]
    fn a_file_changed_outside_updates_its_task_and_a_status_by_name_works() {
        let (mut db, v, dir) = setup();
        let t = lifecycle::create(&mut db, &NewTask { notes: "one".into(), ..NewTask::new("Doc") }).unwrap();
        sync_flush(&mut db, &v);
        let text = read(&dir, "Doc.md").replace("one", "two").replace("status: To do", "status: Done");
        fs::write(dir.join("Doc.md"), text).unwrap();
        let r = sync_flush(&mut db, &v);
        assert_eq!(r.updated, 1, "{r:?}");
        let now = Task::from(&db.get(t.id).unwrap());
        assert_eq!(now.notes, "two");
        assert!(statuses::load(&db).unwrap().is_closed(now.status));
        // A status the person does not have leaves the task as it is.
        let text = read(&dir, "Doc.md").replace("status: Done", "status: Waiting for review");
        fs::write(dir.join("Doc.md"), text).unwrap();
        sync_flush(&mut db, &v);
        assert!(statuses::load(&db).unwrap().is_closed(Task::from(&db.get(t.id).unwrap()).status));
    }

    #[test]
    fn a_change_in_the_app_reaches_the_file_and_a_rename_or_epic_change_moves_it() {
        let (mut db, v, dir) = setup();
        let t = new(&mut db, "Doc");
        sync_flush(&mut db, &v);
        lifecycle::edit(&mut db, t.id, &Edit { title: Some("Renamed".into()), ..Edit::default() }).unwrap();
        flush(&mut db, &v);
        assert!(dir.join("Renamed.md").exists() && !dir.join("Doc.md").exists());
        let e = lifecycle::create_epic(&mut db, "Box").unwrap();
        lifecycle::edit(&mut db, t.id, &Edit { epic: Some(Some(e.id)), ..Edit::default() }).unwrap();
        flush(&mut db, &v);
        assert!(dir.join("Box/Renamed.md").exists() && !dir.join("Renamed.md").exists());
        // Renaming the epic renames the directory.
        lifecycle::edit_epic(&mut db, e.id, &Edit { title: Some("Crate".into()), ..Edit::default() }).unwrap();
        flush(&mut db, &v);
        assert!(dir.join("Crate/Renamed.md").exists() && !dir.join("Box").exists());
        assert!(!sync_flush(&mut db, &v).changed());
    }

    #[test]
    fn a_file_moved_between_epic_directories_moves_its_task_and_its_subtasks() {
        let (mut db, v, dir) = setup();
        let a = lifecycle::create_epic(&mut db, "A").unwrap();
        let b = lifecycle::create_epic(&mut db, "B").unwrap();
        let p = lifecycle::create(&mut db, &NewTask { epic: Some(a.id), ..NewTask::new("Parent") }).unwrap();
        lifecycle::create(&mut db, &NewTask { parent: Some(p.id), epic: Some(a.id), ..NewTask::new("Child") }).unwrap();
        sync_flush(&mut db, &v);
        fs::rename(dir.join("A/Parent"), dir.join("B/Parent")).unwrap();
        let r = sync_flush(&mut db, &v);
        assert!(r.moved >= 1, "{r:?}");
        assert_eq!(Task::from(&db.get(p.id).unwrap()).epic, Some(b.id));
        assert_eq!(task_named(&db, "Child").epic, Some(b.id));
        assert_eq!(titles(&db), ["Child", "Parent"], "no copies");
        assert!(dir.join("B/Parent/Child.md").exists());
    }

    #[test]
    fn a_task_moved_into_another_tasks_directory_becomes_its_subtask() {
        let (mut db, v, dir) = setup();
        let a = new(&mut db, "A");
        let b = new(&mut db, "B");
        sub(&mut db, b.id, "B child");
        sync_flush(&mut db, &v);
        fs::rename(dir.join("A.md"), dir.join("B/A.md")).unwrap();
        sync_flush(&mut db, &v);
        assert_eq!(Task::from(&db.get(a.id).unwrap()).parent_id, Some(b.id));
        assert_eq!(titles(&db).len(), 3);
    }

    #[test]
    fn sections_are_named_in_front_matter_and_in_the_epic_file() {
        let (mut db, v, dir) = setup();
        let e = lifecycle::create_epic(&mut db, "Trip").unwrap();
        let sec = lifecycle::create_section(&mut db, e.id, "Booking").unwrap();
        let t = lifecycle::create(&mut db, &NewTask { epic: Some(e.id), section: Some(sec.id), ..NewTask::new("Flights") }).unwrap();
        sync_flush(&mut db, &v);
        assert!(read(&dir, "Trip/Flights.md").contains("section: Booking"));
        assert!(read(&dir, "Trip/.epic.toml").contains("title = \"Booking\""));
        // A new name in a file makes the section.
        fs::write(dir.join("Trip/Hotel.md"), "---\nsection: Stay\n---\n# Hotel\n").unwrap();
        sync_flush(&mut db, &v);
        let hotel = task_named(&db, "Hotel");
        let stay = db.get(hotel.section.unwrap()).unwrap();
        assert_eq!((stay.title.as_str(), stay.parent_id), ("Stay", Some(e.id)));
        assert_eq!(Task::from(&db.get(t.id).unwrap()).section, Some(sec.id));
    }

    #[test]
    fn a_deleted_file_archives_its_task_which_then_lives_in_the_archive() {
        let (mut db, v, dir) = setup();
        let e = lifecycle::create_epic(&mut db, "Trip").unwrap();
        let t = lifecycle::create(&mut db, &NewTask { epic: Some(e.id), ..NewTask::new("Gone") }).unwrap();
        sync_flush(&mut db, &v);
        fs::remove_file(dir.join("Trip/Gone.md")).unwrap();
        let r = sync_flush(&mut db, &v);
        assert_eq!(r.trashed, 1, "{r:?}");
        assert!(Task::from(&db.get(t.id).unwrap()).archived);
        assert!(dir.join(".archive/Trip/Gone.md").exists());
        assert!(read(&dir, ".archive/Trip/Gone.md").contains("archived: true"));
        // Brought back in the app, it is a file of its epic again.
        lifecycle::set_archived(&mut db, t.id, false).unwrap();
        flush(&mut db, &v);
        assert!(dir.join("Trip/Gone.md").exists() && !dir.join(".archive/Trip/Gone.md").exists());
        // Deleted for good in the app: the file goes.
        let before = v.known(&db).unwrap();
        db.delete(t.id).unwrap();
        assert_eq!(v.flush(&mut db, &before).unwrap().removed, 1);
        assert!(!dir.join("Trip/Gone.md").exists());
    }

    #[test]
    fn an_archived_file_is_an_archived_task() {
        let (mut db, v, dir) = setup();
        fs::create_dir_all(dir.join(".archive/Old")).unwrap();
        fs::write(dir.join(".archive/Old/Done thing.md"), "# Done thing\n").unwrap();
        sync_flush(&mut db, &v);
        let t = task_named(&db, "Done thing");
        assert!(t.archived);
        assert!(t.epic.is_some(), "the epic directory of the archive is the same epic");
        assert!(!sync_flush(&mut db, &v).changed());
    }

    #[test]
    fn plain_files_are_not_read_twice_before_they_are_written() {
        let (mut db, v, dir) = setup();
        fs::write(dir.join("Plain.md"), "just text\n").unwrap();
        fs::write(dir.join("Titled.md"), "# Titled\n").unwrap();
        assert_eq!(v.sync(&mut db).unwrap().created, 2);
        let r = v.sync(&mut db).unwrap();
        assert!(!r.changed(), "{r:?}");
        assert_eq!(titles(&db), ["Plain", "Titled"]);
    }

    #[test]
    fn files_claiming_one_id_are_resolved_by_path_and_the_rest_are_tasks_of_their_own() {
        let (mut db, v, dir) = setup();
        let t = new(&mut db, "Orig");
        sync_flush(&mut db, &v);
        let text = read(&dir, "Orig.md");
        fs::write(dir.join("Zcopy.md"), &text).unwrap();
        fs::write(dir.join("Acopy.md"), &text).unwrap();
        let r = v.sync(&mut db).unwrap();
        assert_eq!(r.created, 2, "{r:?}");
        assert_eq!(db.get(t.id).unwrap().fields["file"], "Acopy.md");
        let known = v.known(&db).unwrap();
        v.flush(&mut db, &known).unwrap();
        let mut files: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".md"))
            .collect();
        files.sort();
        assert_eq!(files, ["Orig-2.md", "Orig-3.md", "Orig.md"], "three tasks, three files, none lost");
        assert!(!sync_flush(&mut db, &v).changed());
        assert_eq!(titles(&db), ["Orig", "Orig", "Orig"]);
    }

    #[test]
    fn when_both_changed_the_file_wins_and_the_task_is_kept_beside_it() {
        let (mut db, v, dir) = setup();
        let t = lifecycle::create(&mut db, &NewTask { notes: "start".into(), ..NewTask::new("Both") }).unwrap();
        sync_flush(&mut db, &v);
        lifecycle::edit(&mut db, t.id, &Edit { notes: Some("app".into()), ..Edit::default() }).unwrap();
        let text = read(&dir, "Both.md").replace("start", "file");
        fs::write(dir.join("Both.md"), text).unwrap();
        let r = v.sync(&mut db).unwrap();
        assert_eq!(r.conflicts, 1, "{r:?}");
        assert_eq!(Task::from(&db.get(t.id).unwrap()).notes, "file");
        let copy = read(&dir, "Both (conflict).md");
        assert!(copy.contains("app") && !copy.contains("id:"), "{copy}");
    }

    #[test]
    fn the_epic_file_round_trips_dates_colour_icon_and_description() {
        let (mut db, v, dir) = setup();
        let e = lifecycle::create_epic(&mut db, "Trip").unwrap();
        lifecycle::edit_epic(
            &mut db,
            e.id,
            &Edit {
                notes: Some("About \"it\"".into()),
                color: Some(Some("#112233".into())),
                icon: Some(Some("star".into())),
                due: Some(Some("2026-12-24".parse().unwrap())),
                ..Edit::default()
            },
        )
        .unwrap();
        flush(&mut db, &v);
        let toml = read(&dir, "Trip/.epic.toml");
        fs::write(dir.join("Trip/.epic.toml"), toml.replace("#112233", "#445566").replace("2026-12-24", "2027-01-02")).unwrap();
        v.sync(&mut db).unwrap();
        let now = Epic::from(&db.get(e.id).unwrap());
        assert_eq!(now.color.as_deref(), Some("#445566"));
        assert_eq!(now.due.map(|d| d.to_string()).as_deref(), Some("2027-01-02"));
        assert_eq!((now.notes.as_str(), now.icon.as_deref()), ("About \"it\"", Some("star")));
    }

    #[test]
    fn adding_a_file_moves_it_in_and_links_are_followed_and_written_through() {
        let (mut db, v, dir) = setup();
        let outside = temp();
        let src = outside.join("Draft.md");
        fs::write(&src, "# Draft\n\nwords\n").unwrap();
        let (dest, linked) = v.add_file(&db, &src, None).unwrap();
        assert!(!linked && dest.exists() && !src.exists());
        let src2 = outside.join("Linked.md");
        fs::write(&src2, "# Linked\n\nold\n").unwrap();
        let at = dir.join("Linked.md");
        link(&src2, &at).unwrap();
        sync_flush(&mut db, &v);
        assert_eq!(titles(&db), ["Draft", "Linked"]);
        let id = task_named(&db, "Linked").id;
        lifecycle::edit(&mut db, id, &Edit { notes: Some("new".into()), ..Edit::default() }).unwrap();
        flush(&mut db, &v);
        assert!(is_symlink(&at));
        assert!(fs::read_to_string(&src2).unwrap().contains("new"));
        let _ = fs::remove_dir_all(outside);
    }

    #[test]
    fn dead_links_are_reported_and_linked_directories_not_followed() {
        let (mut db, v, dir) = setup();
        let outside = temp();
        link(&outside.join("nope.md"), &dir.join("Dead.md")).unwrap();
        fs::write(outside.join("Inside.md"), "# Inside\n").unwrap();
        link(&outside, &dir.join("Linked")).unwrap();
        let r = v.sync(&mut db).unwrap();
        assert!(r.warnings.iter().any(|w| w.contains("Dead.md")), "{r:?}");
        assert!(titles(&db).is_empty());
        let _ = fs::remove_dir_all(outside);
    }

    #[test]
    fn a_vault_may_not_hold_the_database() {
        let dir = temp();
        let db = dir.join("todo.db");
        fs::write(&db, "").unwrap();
        assert!(Vault::open(&dir, Some(&db)).is_err());
    }
}
