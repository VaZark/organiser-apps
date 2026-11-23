//! Notes' state and every action, with no Qt in it: the bridge in `backend.rs` only forwards.
//! What a page shows comes out as JSON that QML reads straight into its models; all decisions
//! (grouping, sections, counts, colours, empty-state text) are made here.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use notes_schema::color::Color;
use notes_schema::folder::{Folder, FolderEdit, FolderSummary};
use notes_schema::lifecycle::{self, NewNote};
use notes_schema::note::{Edit, Note};
use notes_schema::vault::{Import, Known, Vault};
use notes_schema::views::{self, Filter, Scope, Sort};
use organiser_core::settings::Settings;
use organiser_engine::{ChildPolicy, Db, Error, Filters, Result, TagFilter};
use serde_json::{Value, json};

/// The icons a folder can take: what is stored (the names the other Notes frontends store), the
/// theme icon drawn for it, and its label.
pub const ICONS: &[(&str, &str, &str)] = &[
    ("folder", "folder", "Folder"),
    ("note", "view-pim-notes", "Note"),
    ("heart", "emblem-favorite", "Heart"),
    ("moon", "weather-clear-night", "Moon"),
    ("tags", "tag", "Tags"),
    ("pin", "pin", "Pin"),
    ("archive", "folder-archive", "Archive"),
    ("eye", "view-visible", "Eye"),
    ("check", "checkmark", "Done"),
    ("kanban", "view-kanban", "Board"),
    ("grid", "view-grid", "Grid"),
    ("list", "view-list-details", "List"),
    ("rows", "view-list-text", "Rows"),
    ("palette", "color-management", "Palette"),
];

/// The theme icon for a stored name; the folder for none or an unknown one.
pub fn theme_icon(stored: Option<&str>) -> &'static str {
    ICONS
        .iter()
        .find(|(n, _, _)| Some(*n) == stored)
        .map_or("folder", |(_, t, _)| t)
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// FNV-1a, so a folder's colour stays the same from run to run.
fn fnv(s: &str) -> usize {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h as usize
}

fn opt(id: i64) -> Option<i64> {
    (id >= 0).then_some(id)
}

/// What the page shows, the database behind it, and the folder the notes are kept in.
pub struct Core {
    db: Db,
    pub scope: Scope,
    pub text: String,
    /// Notes must have all of these tags.
    pub tag_filter: Vec<String>,
    pub filters: Filters,
    pub sort: Sort,
    pub group_by_folder: bool,
    pub group_by_section: bool,
    pub show_all_notes: bool,
    folders: Vec<FolderSummary>,
    tag_colors: HashMap<String, String>,
    db_path: PathBuf,
    vault: Option<Vault>,
    /// The file each note had after the last flush.
    known: Known,
    /// The next flush is skipped: a freshly loaded folder is only read, not written to.
    skip_flush: bool,
    pub settings: Settings,
    settings_path: Option<PathBuf>,
    /// What the person should be told after the last action.
    pub notice: Option<String>,
}

impl Core {
    /// Open the database and the notes folder the settings name (`vault` overrides it; an empty
    /// path means none). `settings_path` replaces the settings file (for tests).
    pub fn open(
        path: &Path,
        vault: Option<PathBuf>,
        settings_path: Option<PathBuf>,
    ) -> Result<Self> {
        let mut db = Db::open(path, &notes_schema::SCHEMA)?;
        lifecycle::ensure_default_folder(&mut db)?;
        let loaded = match &settings_path {
            Some(p) => Settings::load_from(p),
            None => Settings::load(),
        };
        let settings = loaded.settings;
        let mut notice = loaded.problem;
        if let Err(e) = lifecycle::purge_expired(&mut db, now(), settings.notes.trash_days) {
            notice = Some(format!("cannot empty the old trash: {e}"));
        }
        let mut core = Self {
            db,
            scope: Scope::Notes,
            text: String::new(),
            tag_filter: Vec::new(),
            filters: Filters::default(),
            sort: Sort::default(),
            group_by_folder: settings.notes.group_by_folder,
            group_by_section: settings.notes.group_by_section,
            show_all_notes: settings.notes.show_all_notes,
            folders: Vec::new(),
            tag_colors: HashMap::new(),
            db_path: path.to_path_buf(),
            vault: None,
            known: Known::default(),
            skip_flush: false,
            settings,
            settings_path,
            notice,
        };
        let dir = match vault {
            Some(v) if v.as_os_str().is_empty() => None,
            Some(v) => Some(v),
            None => core.settings.notes.vault_dir.clone(),
        };
        if let Some(dir) = dir {
            core.open_vault(dir, false)?;
        }
        core.refresh()?;
        Ok(core)
    }

    fn save_settings(&self) -> std::result::Result<(), String> {
        match &self.settings_path {
            Some(p) => self.settings.save_to(p),
            None => self.settings.save(),
        }
    }

    // ---- the notes folder (vault) ------------------------------------------------------------

    fn open_vault(&mut self, dir: PathBuf, load_only: bool) -> Result<()> {
        let v = Vault::open(dir, Some(&self.db_path))?;
        let mut report = v.sync(&mut self.db)?;
        if !load_only {
            report.add(v.flush(&mut self.db, &self.known)?);
        } else {
            self.skip_flush = true;
        }
        self.known = v.known(&self.db)?;
        self.notice = report
            .warnings
            .first()
            .cloned()
            .or_else(|| Some(report.summary()));
        self.vault = Some(v);
        Ok(())
    }

    pub fn vault_dir(&self) -> Option<&Path> {
        self.vault.as_ref().map(Vault::dir)
    }

    /// Choose the notes folder (`None`: the database alone). Choosing a different folder loads it
    /// and drops what the app held, so nothing is carried over and written into it.
    pub fn set_vault(&mut self, dir: Option<PathBuf>) -> Result<()> {
        let same = self.vault.as_ref().is_some_and(|v| {
            dir.as_ref()
                .is_some_and(|d| std::fs::canonicalize(d).ok().as_deref() == Some(v.dir()))
        });
        if dir.is_some() && !same {
            Vault::forget(&mut self.db)?;
            self.known = Known::default();
        }
        self.settings.notes.vault_dir = dir.clone();
        self.vault = None;
        let result = match dir {
            Some(d) => self.open_vault(d, !same),
            None => {
                self.known = Known::default();
                self.notice =
                    Some("The notes are kept in the database alone; the files stay".into());
                Ok(())
            }
        };
        if result.is_err() {
            self.settings.notes.vault_dir = None;
        }
        let saved = self.save_settings();
        result?;
        saved.map_err(Error::Invalid)
    }

    /// The folder `Notes` in the documents folder.
    pub fn default_vault_dir() -> PathBuf {
        organiser_core::paths::documents_folder(&notes_schema::META)
    }

    /// Read the folder's files into the notes (changes made outside the app), then write back.
    pub fn sync_vault(&mut self) -> Result<()> {
        let Some(v) = self.vault.clone() else {
            return Ok(());
        };
        let mut report = v.sync(&mut self.db)?;
        report.add(v.flush(&mut self.db, &self.known)?);
        self.known = v.known(&self.db)?;
        self.notice = Some(
            report
                .warnings
                .first()
                .cloned()
                .unwrap_or_else(|| report.summary()),
        );
        Ok(())
    }

    /// Write the database's changes to the folder. Called after every action.
    pub fn flush(&mut self) -> Result<()> {
        if std::mem::take(&mut self.skip_flush) {
            return Ok(());
        }
        let Some(v) = self.vault.clone() else {
            return Ok(());
        };
        let report = v.flush(&mut self.db, &self.known)?;
        self.known = v.known(&self.db)?;
        if let Some(w) = report.warnings.first() {
            self.notice = Some(w.clone());
        }
        Ok(())
    }

    /// Bring Markdown files (or folders of them) into the vault, in `folder`, as copies or links.
    pub fn import(&mut self, paths: &[PathBuf], link: bool, folder: Option<i64>) -> Result<usize> {
        let Some(v) = self.vault.clone() else {
            return Err(Error::Invalid("choose a notes folder first".into()));
        };
        let how = if link { Import::Link } else { Import::Copy };
        let mut done = 0;
        for p in paths {
            done += if p.is_dir() {
                v.import_dir(&self.db, p, folder, how)?
            } else {
                v.import_file(&self.db, p, folder, how)?;
                1
            };
        }
        self.sync_vault()?;
        self.notice = Some(format!(
            "Imported {done} file{} as {}",
            if done == 1 { "" } else { "s" },
            if link { "links" } else { "copies" }
        ));
        Ok(done)
    }

    /// The directory of a note's file, to show in the file manager.
    pub fn note_dir(&self, id: i64) -> Option<PathBuf> {
        let v = self.vault.as_ref()?;
        let file = v.file_of(&self.db, id)?;
        Some(file.parent().unwrap_or(v.dir()).to_path_buf())
    }

    // ---- what is shown -----------------------------------------------------------------------

    /// `notes`, `favorites`, `folder`, `archive` or `trash`.
    pub fn set_scope(&mut self, name: &str, folder: i64) {
        self.scope = match name {
            "favorites" => Scope::Favorites,
            "archive" => Scope::Archive,
            "trash" => Scope::Trash,
            "folder" => Scope::Folder(folder),
            _ => Scope::Notes,
        };
    }

    pub fn set_sort(&mut self, name: &str) {
        self.sort = match name {
            "created" => Sort::Created,
            "title" => Sort::Title,
            "rating" => Sort::Rating,
            _ => Sort::Recent,
        };
    }

    pub fn sort_name(&self) -> &'static str {
        match self.sort {
            Sort::Recent => "recent",
            Sort::Created => "created",
            Sort::Title => "title",
            Sort::Rating => "rating",
        }
    }

    fn searching(&self) -> bool {
        !self.text.trim().is_empty()
    }

    fn filter(&self) -> Filter {
        Filter {
            scope: self.scope,
            text: self.searching().then(|| self.text.trim().to_string()),
            tags: TagFilter {
                all: self.tag_filter.clone(),
                ..TagFilter::default()
            },
            fields: self.filters.clone(),
            // A search looks in the archive too, so an old note is still found.
            include_archived: self.searching(),
            recursive: matches!(self.scope, Scope::Folder(_))
                && !self.group_by_section
                && self.show_all_notes,
            sort: self.sort,
        }
    }

    /// How many filters narrow the list: tags and the schema's own.
    pub fn filter_count(&self) -> usize {
        self.tag_filter.len() + self.filters.count()
    }

    /// Re-read the folders and tags; a folder that is gone falls back to all the notes, a tag
    /// that is gone no longer filters.
    pub fn refresh(&mut self) -> Result<()> {
        self.folders = views::folders(&self.db)?;
        if let Scope::Folder(id) = self.scope
            && !self.folders.iter().any(|s| s.folder.id == id)
        {
            self.scope = Scope::Notes;
        }
        let palette = self.settings.palette();
        self.db.ensure_tag_colors(&palette)?;
        let tags = self.db.tags()?;
        self.tag_colors = tags
            .iter()
            .filter_map(|t| Some((t.name.clone(), t.color.clone()?)))
            .collect();
        self.tag_filter
            .retain(|f| tags.iter().any(|t| &t.name == f));
        Ok(())
    }

    pub fn scope_name(&self) -> &'static str {
        match self.scope {
            Scope::Notes => "notes",
            Scope::Favorites => "favorites",
            Scope::Folder(_) => "folder",
            Scope::Archive => "archive",
            Scope::Trash => "trash",
        }
    }

    pub fn scope_folder(&self) -> i64 {
        match self.scope {
            Scope::Folder(id) => id,
            _ => -1,
        }
    }

    fn summary(&self, id: i64) -> Option<&FolderSummary> {
        self.folders.iter().find(|s| s.folder.id == id)
    }

    fn children_of(&self, parent: Option<i64>) -> Vec<&FolderSummary> {
        self.folders
            .iter()
            .filter(|s| s.folder.parent == parent)
            .collect()
    }

    fn default_folder(&self) -> Option<i64> {
        self.folders
            .iter()
            .find(|s| s.folder.is_default)
            .map(|s| s.folder.id)
    }

    /// The folders from the top down to `id`, itself last.
    fn path(&self, id: i64) -> Vec<&FolderSummary> {
        let mut path = Vec::new();
        let mut at = Some(id);
        while let Some(s) = at.and_then(|i| self.summary(i)) {
            at = s.folder.parent;
            path.push(s);
            if path.len() > 256 {
                break;
            }
        }
        path.reverse();
        path
    }

    /// Where a folder sits, as `top/sub/folder`. A deep path keeps its first and last folder and
    /// hides what is between: `top/../folder`.
    fn path_label(&self, id: i64) -> String {
        let titles: Vec<&str> = self.path(id).iter().map(|s| s.folder.title.as_str()).collect();
        match titles.as_slice() {
            [first, _, _, .., last] => format!("{first}/../{last}"),
            all => all.join("/"),
        }
    }

    /// The top folder a folder is in (itself, at the top).
    fn base_folder(&self, id: Option<i64>) -> Option<i64> {
        self.path(id?).first().map(|s| s.folder.id)
    }

    /// A folder's colour: its own, or one made from its name so folders tell apart at a glance;
    /// the default folder is plain (empty) unless coloured.
    fn folder_color(&self, f: &Folder) -> String {
        if let Some(c) = f.color {
            return c.hex();
        }
        if f.is_default {
            return String::new();
        }
        let palette = self.settings.palette();
        palette
            .get(fnv(&f.title) % palette.len().max(1))
            .map_or_else(String::new, |s| (*s).to_string())
    }

    fn folder_value(&self, s: &FolderSummary) -> Value {
        let f = &s.folder;
        json!({
            "id": f.id,
            "title": f.title,
            "notes": f.notes,
            "rated": f.rated,
            "color": self.folder_color(f),
            "ownColor": f.color.map(|c| c.hex()),
            "icon": theme_icon(f.icon.as_deref()),
            "iconName": f.icon.clone().unwrap_or_else(|| "folder".into()),
            "isDefault": f.is_default,
            "parent": f.parent,
            "count": s.notes,
            "total": s.total,
            "favorites": s.favorites,
            "subfolders": s.subfolders,
        })
    }

    /// One note as the page draws it. `chip` is the folder shown on it, when one is.
    fn note_value(&self, n: &Note, chip: bool) -> Value {
        let folder = n.folder_id.and_then(|i| self.summary(i)).map(|s| &s.folder);
        let own = n.color;
        let tags: Vec<Value> = n
            .tags
            .iter()
            .map(|t| json!({"name": t, "color": self.tag_colors.get(t)}))
            .collect();
        let excerpt: String = n
            .body
            .lines()
            .take(12)
            .collect::<Vec<_>>()
            .join("\n")
            .chars()
            .take(400)
            .collect();
        json!({
            "id": n.id,
            "title": n.title,
            "body": n.body,
            "excerpt": excerpt,
            "color": own.map(|c| c.hex()).unwrap_or_default(),
            "darkText": own.is_some_and(Color::prefers_dark_text),
            "pinned": n.pinned,
            "favorite": n.favorite,
            "rating": n.rating.unwrap_or(0),
            "rated": folder.is_some_and(|f| f.rated),
            "archived": n.archived,
            "trashed": n.is_trashed(),
            "tags": tags,
            "folderId": n.folder_id,
            "folderTitle": folder.map(|f| f.title.clone()),
            "folderPath": n.folder_id.map(|i| self.path_label(i)),
            "folderColor": folder.map(|f| self.folder_color(f)),
            "folderIcon": folder.map(|f| theme_icon(f.icon.as_deref())),
            "chip": chip && folder.is_some(),
            "created": n.created,
            "updated": n.updated,
        })
    }

    fn notes_value(&self, notes: &[&Note], chip: bool) -> Vec<Value> {
        notes.iter().map(|n| self.note_value(n, chip)).collect()
    }

    /// Whether the page lists every note and so may group them by folder.
    fn grouped_by_folder(&self) -> bool {
        self.group_by_folder
            && matches!(
                self.scope,
                Scope::Notes | Scope::Favorites | Scope::Archive | Scope::Trash
            )
            && !self.folders.is_empty()
    }

    /// The notes of a folder's sub-folders, for a folder's page grouped by section.
    fn sub_notes(&self, id: i64) -> Vec<Note> {
        let f = Filter {
            scope: Scope::Folder(id),
            recursive: self.show_all_notes,
            ..self.filter()
        };
        views::notes(&self.db, &f).unwrap_or_default()
    }

    /// The text of an empty page.
    fn empty(&self) -> Value {
        let (icon, title, hint) = if self.searching() {
            ("system-search", "No notes found", "Try another word.")
        } else {
            match self.scope {
                Scope::Notes => (
                    "view-pim-notes",
                    "No notes yet",
                    "Notes you add appear here.",
                ),
                Scope::Favorites => (
                    "emblem-favorite",
                    "No favourites yet",
                    "Mark a note as a favourite to find it here.",
                ),
                Scope::Folder(_) => (
                    "folder",
                    "Nothing in this folder",
                    "Add a note with the New Note action.",
                ),
                Scope::Archive => (
                    "folder-archive",
                    "Nothing archived",
                    "Archived notes stay out of the way.",
                ),
                Scope::Trash => (
                    "user-trash",
                    "The trash is empty",
                    "Deleted notes wait here for a while.",
                ),
            }
        };
        json!({"icon": icon, "title": title, "hint": hint})
    }

    /// Everything the page draws, decided here: the header of a folder, its sub-folders as
    /// cards or as groups, pinned notes, groups by folder, and the notes.
    pub fn page_json(&self) -> Result<String> {
        let notes = views::notes(&self.db, &self.filter())?;
        let pin_split = matches!(
            self.scope,
            Scope::Notes | Scope::Favorites | Scope::Folder(_)
        );
        let searching = self.searching() || self.filter_count() > 0;
        let here = match self.scope {
            Scope::Folder(id) => Some(id),
            _ => None,
        };
        let sectioned: Vec<&FolderSummary> = match here {
            Some(id) if self.group_by_section => self.children_of(Some(id)),
            _ => Vec::new(),
        };
        let cards: Vec<Value> = match here {
            Some(id) if !self.group_by_section && !self.show_all_notes && !searching => self
                .children_of(Some(id))
                .into_iter()
                .map(|s| self.folder_value(s))
                .collect(),
            _ => Vec::new(),
        };
        // A folder's own notes are drawn with the folder they are in as a chip when they came
        // from a folder below it; elsewhere, when the page is not grouped.
        let chip = match here {
            Some(_) => !self.group_by_section,
            None => !self.grouped_by_folder(),
        };
        let mut pinned: Vec<&Note> = Vec::new();
        let mut others: Vec<&Note> = Vec::new();
        for n in &notes {
            if pin_split && n.pinned && sectioned.is_empty() {
                pinned.push(n);
            } else {
                others.push(n);
            }
        }
        let mut groups: Vec<Value> = Vec::new();
        let mut flat: Vec<Value> = Vec::new();
        if !sectioned.is_empty() {
            flat = self.notes_value(&others, false);
            for s in &sectioned {
                let mine = self.sub_notes(s.folder.id);
                // Searching or filtering shows only the folders that have a match.
                if mine.is_empty() && searching {
                    continue;
                }
                let refs: Vec<&Note> = mine.iter().collect();
                let deeper: Vec<Value> = if searching || self.show_all_notes {
                    Vec::new()
                } else {
                    self.children_of(Some(s.folder.id))
                        .into_iter()
                        .map(|c| self.folder_value(c))
                        .collect()
                };
                groups.push(json!({
                    "folder": self.folder_value(s),
                    "count": mine.len(),
                    "cards": deeper,
                    "notes": self.notes_value(&refs, false),
                }));
            }
        } else if self.grouped_by_folder() {
            let home = self.default_folder();
            let order: Vec<i64> = self.children_of(None).iter().map(|s| s.folder.id).collect();
            for id in order {
                let mine: Vec<&Note> = others
                    .iter()
                    .copied()
                    .filter(|n| self.base_folder(n.folder_id).or(home) == Some(id))
                    .collect();
                if mine.is_empty() {
                    continue;
                }
                let folder = self.summary(id).map(|s| self.folder_value(s));
                groups.push(json!({
                    "folder": folder,
                    "count": mine.len(),
                    "cards": [],
                    // A note in a folder inside this one says which, as a chip.
                    "notes": mine
                        .iter()
                        .map(|n| self.note_value(n, n.folder_id != Some(id)))
                        .collect::<Vec<_>>(),
                }));
            }
        } else {
            flat = self.notes_value(&others, chip);
        }
        let header = here.and_then(|id| {
            let s = self.summary(id)?;
            let mut v = self.folder_value(s);
            let path: Vec<Value> = self
                .path(id)
                .iter()
                .map(|p| json!({"id": p.folder.id, "title": p.folder.title}))
                .collect();
            v["path"] = Value::Array(path);
            Some(v)
        });
        let empty = notes.is_empty() && sectioned.is_empty() && cards.is_empty();
        let page = json!({
            "scope": self.scope_name(),
            "folderId": self.scope_folder(),
            "header": header,
            "cards": cards,
            "pinned": self.notes_value(&pinned, chip),
            "groups": groups,
            "notes": flat,
            "empty": empty.then(|| self.empty()),
            "trashNotice": (self.scope == Scope::Trash && !notes.is_empty()).then(|| format!(
                "Notes in the trash are deleted after {} days.",
                self.settings.notes.trash_days
            )),
            "canGroup": matches!(self.scope, Scope::Notes | Scope::Favorites | Scope::Archive | Scope::Trash)
                && !self.folders.is_empty(),
            "count": notes.len(),
            "filterCount": self.filter_count(),
        });
        Ok(page.to_string())
    }

    /// Every folder in the order a tree lists them (each folder, then those in it), with how
    /// deep it is.
    pub fn folders_json(&self) -> String {
        fn walk(c: &Core, parent: Option<i64>, depth: usize, out: &mut Vec<Value>) {
            for s in c.children_of(parent) {
                let mut v = c.folder_value(s);
                v["depth"] = json!(depth);
                out.push(v);
                if depth < 64 {
                    walk(c, Some(s.folder.id), depth + 1, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(self, None, 0, &mut out);
        Value::Array(out).to_string()
    }

    // ---- notes -------------------------------------------------------------------------------

    pub fn note_json(&self, id: i64) -> Result<String> {
        let item = self.db.get(id)?;
        let mut v = self.note_value(&Note::from(&item), false);
        // The rating shows only where the folder offers one.
        if !v["rated"].as_bool().unwrap_or(false) {
            v["rating"] = json!(0);
        }
        Ok(v.to_string())
    }

    /// A note being read: its file gets its front matter the first time.
    pub fn open_note(&mut self, id: i64) -> Result<()> {
        if let Some(v) = self.vault.clone() {
            v.stamp(&mut self.db, id)?;
            self.known = v.known(&self.db)?;
        }
        Ok(())
    }

    /// Create a note from a draft: `{title, body, color, tags, folder, favorite, rating,
    /// pinned}`. A title is needed; a rating is kept only where the folder offers them.
    pub fn create_note(&mut self, draft: &str) -> Result<i64> {
        let d: Value = serde_json::from_str(draft).map_err(|e| Error::Invalid(e.to_string()))?;
        let title = d["title"].as_str().unwrap_or("").trim().to_string();
        if title.is_empty() {
            return Err(Error::Invalid("Give the note a title first.".into()));
        }
        let folder = d["folder"]
            .as_i64()
            .and_then(opt)
            .or_else(|| match self.scope {
                Scope::Folder(id) => Some(id),
                _ => self.default_folder(),
            });
        let rated = folder.is_some_and(|id| {
            self.folders
                .iter()
                .any(|s| s.folder.id == id && s.folder.rated)
        });
        let rating = d["rating"]
            .as_u64()
            .filter(|r| (1..=5).contains(r))
            .map(|r| r as u8);
        let new = NewNote {
            title,
            body: d["body"].as_str().unwrap_or("").trim_end().to_string(),
            color: d["color"].as_str().and_then(Color::parse),
            tags: d["tags"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(clean_tag)
                        .filter(|t| !t.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            folder,
            favorite: d["favorite"].as_bool().unwrap_or(false),
            rating: rating.filter(|_| rated),
        };
        let n = lifecycle::create(&mut self.db, &new)?;
        if d["pinned"].as_bool().unwrap_or(false) {
            lifecycle::set_pinned(&mut self.db, n.id, true)?;
        }
        // A new note belongs in the notes, whatever the page was showing; in a folder's own page
        // (or the favourites, when it is one) it stays there.
        if !matches!(self.scope, Scope::Folder(_) | Scope::Favorites) {
            self.scope = Scope::Notes;
        }
        Ok(n.id)
    }

    /// An emptied title keeps the old one: a note must have a title.
    pub fn edit_note(&mut self, id: i64, title: &str, body: &str) -> Result<()> {
        let edit = Edit {
            title: (!title.trim().is_empty()).then(|| title.to_string()),
            body: Some(body.trim_end().to_string()),
            ..Edit::default()
        };
        lifecycle::edit(&mut self.db, id, &edit).map(|_| ())
    }

    /// `#rrggbb` (or `rgb`); empty clears the colour.
    pub fn set_note_color(&mut self, id: i64, color: &str) -> Result<()> {
        let c = match color.trim() {
            "" => None,
            t => Some(
                Color::parse(t).ok_or_else(|| Error::Invalid(format!("{t} is not a colour")))?,
            ),
        };
        let edit = Edit {
            color: Some(c),
            ..Edit::default()
        };
        lifecycle::edit(&mut self.db, id, &edit).map(|_| ())
    }

    pub fn set_pinned(&mut self, id: i64, on: bool) -> Result<()> {
        lifecycle::set_pinned(&mut self.db, id, on).map(|_| ())
    }

    pub fn set_favorite(&mut self, id: i64, on: bool) -> Result<()> {
        lifecycle::set_favorite(&mut self.db, id, on).map(|_| ())
    }

    /// 1 to 5 stars; 0 clears.
    pub fn set_rating(&mut self, id: i64, stars: i64) -> Result<()> {
        let edit = Edit {
            rating: Some((1..=5).contains(&stars).then_some(stars as u8)),
            ..Edit::default()
        };
        lifecycle::edit(&mut self.db, id, &edit).map(|_| ())
    }

    pub fn set_archived(&mut self, id: i64, on: bool) -> Result<()> {
        lifecycle::set_archived(&mut self.db, id, on).map(|_| ())
    }

    pub fn trash(&mut self, id: i64) -> Result<()> {
        lifecycle::trash(&mut self.db, id, now())?;
        self.notice = Some("Moved to the trash".into());
        Ok(())
    }

    pub fn restore(&mut self, id: i64) -> Result<()> {
        lifecycle::restore(&mut self.db, id).map(|_| ())
    }

    pub fn purge(&mut self, id: i64) -> Result<()> {
        lifecycle::purge(&mut self.db, id)
    }

    pub fn empty_trash(&mut self) -> Result<()> {
        let n = lifecycle::empty_trash(&mut self.db)?;
        self.notice = Some(format!("Deleted {n} note{}", if n == 1 { "" } else { "s" }));
        Ok(())
    }

    pub fn set_tags(&mut self, id: i64, tags: &[String]) -> Result<()> {
        let tags: Vec<String> = tags
            .iter()
            .map(|t| clean_tag(t))
            .filter(|t| !t.is_empty())
            .collect();
        lifecycle::set_tags(&mut self.db, id, &tags)
    }

    /// File a note in a folder (`None`: the default one). When the page showed the folder it
    /// left, the page follows it so it does not vanish from view.
    pub fn move_note(&mut self, id: i64, folder: Option<i64>) -> Result<()> {
        lifecycle::move_note(&mut self.db, id, folder)?;
        if matches!(self.scope, Scope::Folder(_)) {
            self.scope = folder.map_or(Scope::Notes, Scope::Folder);
        }
        Ok(())
    }

    // ---- folders -----------------------------------------------------------------------------

    /// A folder's settings: its fields, where it can move to, and the icons to pick from.
    pub fn folder_json(&self, id: i64) -> Result<String> {
        let s = self
            .summary(id)
            .ok_or_else(|| Error::Invalid(format!("no folder {id}")))?;
        let mut below = std::collections::HashSet::new();
        let mut next = vec![id];
        while let Some(at) = next.pop() {
            for c in self.children_of(Some(at)) {
                if below.insert(c.folder.id) {
                    next.push(c.folder.id);
                }
            }
        }
        let mut choices = vec![json!({"id": -1, "title": "Top level"})];
        for f in self.folders_value_list() {
            let fid = f["id"].as_i64().unwrap_or(-1);
            if fid == id || below.contains(&fid) || f["isDefault"] == true {
                continue;
            }
            let depth = f["depth"].as_u64().unwrap_or(0) as usize;
            choices.push(json!({
                "id": fid,
                "title": format!("{}{}", "\u{2003}".repeat(depth), f["title"].as_str().unwrap_or("")),
            }));
        }
        let mut v = self.folder_value(s);
        v["parentChoices"] = Value::Array(choices);
        v["icons"] = Value::Array(
            ICONS
                .iter()
                .map(|(n, t, l)| json!({"name": n, "icon": t, "label": l}))
                .collect(),
        );
        Ok(v.to_string())
    }

    fn folders_value_list(&self) -> Vec<Value> {
        serde_json::from_str(&self.folders_json()).unwrap_or_default()
    }

    /// A new folder in `parent` (or at the top). Returns its id.
    pub fn create_folder(&mut self, parent: Option<i64>) -> Result<i64> {
        Ok(lifecycle::create_folder_in(&mut self.db, parent, "New folder", false)?.id)
    }

    fn edit_folder(&mut self, id: i64, edit: FolderEdit) -> Result<()> {
        lifecycle::edit_folder(&mut self.db, id, &edit).map(|_| ())
    }

    /// An empty name is ignored: the old one stays until there is a new one.
    pub fn set_folder_title(&mut self, id: i64, title: &str) -> Result<()> {
        if title.trim().is_empty() {
            return Ok(());
        }
        self.edit_folder(
            id,
            FolderEdit {
                title: Some(title.trim().to_string()),
                ..FolderEdit::default()
            },
        )
    }

    pub fn set_folder_notes(&mut self, id: i64, notes: &str) -> Result<()> {
        self.edit_folder(
            id,
            FolderEdit {
                notes: Some(notes.to_string()),
                ..FolderEdit::default()
            },
        )
    }

    pub fn set_folder_rated(&mut self, id: i64, rated: bool) -> Result<()> {
        self.edit_folder(
            id,
            FolderEdit {
                rated: Some(rated),
                ..FolderEdit::default()
            },
        )
    }

    /// The stored name of an icon; empty is the folder.
    pub fn set_folder_icon(&mut self, id: i64, name: &str) -> Result<()> {
        let icon = (!name.is_empty() && name != "folder").then(|| name.to_string());
        self.edit_folder(
            id,
            FolderEdit {
                icon: Some(icon),
                ..FolderEdit::default()
            },
        )
    }

    /// `#rrggbb` (or `rgb`); empty clears it. Text that is not a colour is an error.
    pub fn set_folder_color(&mut self, id: i64, color: &str) -> Result<()> {
        let c = match color.trim() {
            "" => None,
            t => Some(
                Color::parse(t).ok_or_else(|| Error::Invalid(format!("{t} is not a colour")))?,
            ),
        };
        self.edit_folder(
            id,
            FolderEdit {
                color: Some(c),
                ..FolderEdit::default()
            },
        )
    }

    pub fn move_folder_into(&mut self, id: i64, parent: Option<i64>) -> Result<()> {
        lifecycle::move_folder_into(&mut self.db, id, parent)
    }

    /// Delete a folder, keeping its notes and folders (in the folder it was in), or deleting
    /// them with it.
    pub fn delete_folder(&mut self, id: i64, with_notes: bool) -> Result<()> {
        self.notice = Some(if with_notes {
            let n = lifecycle::delete_folder_and_notes(&mut self.db, id)?;
            format!(
                "Deleted the folder and {n} note{}",
                if n == 1 { "" } else { "s" }
            )
        } else {
            lifecycle::delete_folder(&mut self.db, id)?;
            "Deleted the folder; its notes and folders stay".into()
        });
        Ok(())
    }

    // ---- filters -----------------------------------------------------------------------------

    pub fn filters_json(&self) -> String {
        let any = |f: &str| -> Vec<Value> { self.filters.any_of(f).to_vec() };
        json!({
            "count": self.filter_count(),
            "pinned": !any("pinned").is_empty(),
            "favorite": !any("favorite").is_empty(),
            "ratings": any("rating"),
            "tags": self.tag_filter,
        })
        .to_string()
    }

    /// Toggle one of the schema's filters: `pinned` or `favorite` (on or off), `rating` (a
    /// star count).
    pub fn toggle_filter(&mut self, field: &str, value: i64) {
        match field {
            "pinned" | "favorite" => {
                self.filters.remove(field);
                if value != 0 {
                    self.filters.toggle(field, Value::Bool(true));
                }
            }
            "rating" => self.filters.toggle(field, json!(value)),
            _ => {}
        }
    }

    /// Add the tag to the filter, or take it out.
    pub fn toggle_tag_filter(&mut self, name: &str) {
        match self.tag_filter.iter().position(|t| t == name) {
            Some(i) => {
                self.tag_filter.remove(i);
            }
            None => self.tag_filter.push(name.to_string()),
        }
    }

    pub fn clear_filters(&mut self) {
        self.filters.clear();
        self.tag_filter.clear();
    }

    // ---- tags --------------------------------------------------------------------------------

    pub fn tags_json(&self) -> Result<String> {
        let counts = views::notes(&self.db, &Filter::default())?
            .iter()
            .flat_map(|n| n.tags.clone())
            .fold(HashMap::<String, usize>::new(), |mut m, t| {
                *m.entry(t).or_default() += 1;
                m
            });
        let tags: Vec<Value> = self
            .db
            .tags()?
            .into_iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "color": t.color,
                    "parent": t.parent,
                    "count": counts.get(&t.name).copied().unwrap_or(0),
                })
            })
            .collect();
        Ok(Value::Array(tags).to_string())
    }

    pub fn create_tag(&mut self, name: &str, parent: &str) -> Result<()> {
        self.db
            .create_tag(&clean_tag(name), (!parent.is_empty()).then_some(parent))?;
        Ok(())
    }

    pub fn set_tag_color(&mut self, name: &str, color: &str) -> Result<()> {
        self.db
            .set_tag_color(name, (!color.is_empty()).then_some(color))?;
        Ok(())
    }

    pub fn set_tag_parent(&mut self, name: &str, parent: &str) -> Result<()> {
        self.db
            .set_tag_parent(name, (!parent.is_empty()).then_some(parent))?;
        Ok(())
    }

    pub fn delete_tag(&mut self, name: &str, with_children: bool) -> Result<()> {
        let policy = if with_children {
            ChildPolicy::Delete
        } else {
            ChildPolicy::Detach
        };
        self.db.delete_tag(name, policy)?;
        Ok(())
    }

    // ---- preferences -------------------------------------------------------------------------

    pub fn prefs_json(&self) -> String {
        let n = &self.settings.notes;
        json!({
            "trashDays": n.trash_days,
            "listView": n.list_view,
            "detailsInDrawer": n.details_in_drawer,
            "groupByFolder": n.group_by_folder,
            "groupBySection": n.group_by_section,
            "showAllNotes": n.show_all_notes,
            "presets": notes_schema::color::PRESETS
                .iter()
                .map(|(name, hex)| json!({"name": name, "hex": hex}))
                .collect::<Vec<_>>(),
            "vaultDir": self.vault_dir().map(|d| d.display().to_string()).unwrap_or_default(),
        })
        .to_string()
    }

    /// Change a preference and remember it: `listView`, `detailsInDrawer`, `groupByFolder`,
    /// `groupBySection` or `showAllNotes` (those that group also change what the page shows now).
    pub fn set_pref_bool(&mut self, key: &str, on: bool) -> std::result::Result<(), String> {
        let n = &mut self.settings.notes;
        match key {
            "listView" => n.list_view = on,
            "detailsInDrawer" => n.details_in_drawer = on,
            "groupByFolder" => {
                n.group_by_folder = on;
                self.group_by_folder = on;
            }
            "groupBySection" => {
                n.group_by_section = on;
                self.group_by_section = on;
            }
            "showAllNotes" => {
                n.show_all_notes = on;
                self.show_all_notes = on;
            }
            _ => return Err(format!("no preference {key}")),
        }
        self.save_settings()
    }

    pub fn set_trash_days(&mut self, days: u32) -> std::result::Result<(), String> {
        self.settings.notes.trash_days = days.clamp(1, 3650);
        self.save_settings()
    }
}

/// A tag name as typed: without a leading `#` or `@`.
fn clean_tag(t: &str) -> String {
    t.trim().trim_start_matches(['#', '@']).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "notes-kirigami-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// No folder of notes, and settings kept in a temp file, whatever the person's own say.
    fn core() -> Core {
        let d = temp("db");
        Core::open(&d.join("t.db"), Some(PathBuf::new()), Some(d.join("c.ron"))).unwrap()
    }

    fn page(c: &Core) -> Value {
        serde_json::from_str(&c.page_json().unwrap()).unwrap()
    }

    fn make(c: &mut Core, title: &str) -> i64 {
        let id = c.create_note(&json!({"title": title}).to_string()).unwrap();
        c.refresh().unwrap();
        id
    }

    fn titles(v: &Value) -> Vec<String> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|n| n["title"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn a_deep_folder_path_keeps_only_its_first_and_last_folder() {
        let mut c = core();
        let a = c.create_folder(None).unwrap();
        let b = c.create_folder(Some(a)).unwrap();
        let d = c.create_folder(Some(b)).unwrap();
        let e = c.create_folder(Some(d)).unwrap();
        for (id, name) in [(a, "A"), (b, "B"), (d, "D"), (e, "E")] {
            c.set_folder_title(id, name).unwrap();
        }
        c.refresh().unwrap();
        assert_eq!(c.path_label(a), "A");
        assert_eq!(c.path_label(b), "A/B");
        assert_eq!(c.path_label(d), "A/B/D");
        assert_eq!(c.path_label(e), "A/../E");
    }

    #[test]
    fn a_new_note_needs_a_title_and_lands_in_unassigned() {
        let mut c = core();
        assert!(c.create_note(r#"{"title":"  "}"#).is_err());
        let id = make(&mut c, "idea");
        let n: Value = serde_json::from_str(&c.note_json(id).unwrap()).unwrap();
        assert_eq!(n["folderTitle"], "Unassigned");
        let p = page(&c);
        assert!(p["empty"].is_null());
        assert_eq!(p["count"], 1);
    }

    #[test]
    fn an_empty_page_says_why() {
        let mut c = core();
        assert_eq!(page(&c)["empty"]["title"], "No notes yet");
        c.set_scope("trash", -1);
        assert_eq!(page(&c)["empty"]["title"], "The trash is empty");
        c.text = "zzz".into();
        assert_eq!(page(&c)["empty"]["title"], "No notes found");
    }

    #[test]
    fn pinned_notes_split_off_and_trash_restore_purge_work() {
        let mut c = core();
        c.group_by_folder = false;
        let a = make(&mut c, "a");
        make(&mut c, "b");
        c.set_pinned(a, true).unwrap();
        let p = page(&c);
        assert_eq!(titles(&p["pinned"]), ["a"]);
        assert_eq!(titles(&p["notes"]), ["b"]);
        c.trash(a).unwrap();
        c.refresh().unwrap();
        assert_eq!(page(&c)["count"], 1);
        c.set_scope("trash", -1);
        let p = page(&c);
        assert_eq!(titles(&p["notes"]), ["a"]);
        assert!(p["trashNotice"].is_string());
        c.restore(a).unwrap();
        c.trash(a).unwrap();
        c.purge(a).unwrap();
        assert_eq!(page(&c)["count"], 0);
    }

    #[test]
    fn notes_group_by_their_top_folder() {
        let mut c = core();
        let work = c.create_folder(None).unwrap();
        c.set_folder_title(work, "Work").unwrap();
        let sub = c.create_folder(Some(work)).unwrap();
        c.refresh().unwrap();
        c.create_note(&json!({"title": "plan", "folder": sub}).to_string())
            .unwrap();
        make(&mut c, "loose");
        let p = page(&c);
        let groups = p["groups"].as_array().unwrap();
        let names: Vec<&str> = groups
            .iter()
            .map(|g| g["folder"]["title"].as_str().unwrap())
            .collect();
        // The note in the folder inside Work is in Work's group.
        assert_eq!(names, ["Work", "Unassigned"]);
        assert_eq!(groups[0]["notes"][0]["title"], "plan");
        c.group_by_folder = false;
        let p = page(&c);
        assert!(p["groups"].as_array().unwrap().is_empty());
        assert_eq!(p["notes"].as_array().unwrap().len(), 2);
        assert_eq!(p["notes"][0]["chip"], true);
    }

    #[test]
    fn a_folders_page_shows_sub_folders_as_cards_or_as_groups() {
        let mut c = core();
        let work = c.create_folder(None).unwrap();
        let sub = c.create_folder(Some(work)).unwrap();
        c.set_folder_title(sub, "Drafts").unwrap();
        c.refresh().unwrap();
        c.create_note(&json!({"title": "d1", "folder": sub}).to_string())
            .unwrap();
        c.create_note(&json!({"title": "w1", "folder": work}).to_string())
            .unwrap();
        c.set_scope("folder", work);
        c.group_by_section = false;
        let p = page(&c);
        assert_eq!(p["header"]["id"], work);
        assert_eq!(p["cards"][0]["title"], "Drafts");
        assert_eq!(titles(&p["notes"]), ["w1"]);
        c.show_all_notes = true;
        let p = page(&c);
        assert!(p["cards"].as_array().unwrap().is_empty());
        assert_eq!(p["notes"].as_array().unwrap().len(), 2);
        c.show_all_notes = false;
        c.group_by_section = true;
        let p = page(&c);
        assert_eq!(titles(&p["notes"]), ["w1"]);
        assert_eq!(p["groups"][0]["folder"]["title"], "Drafts");
        assert_eq!(p["groups"][0]["notes"][0]["title"], "d1");
        assert_eq!(p["header"]["path"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn ratings_show_only_in_rated_folders_and_filters_narrow() {
        let mut c = core();
        let f = c.create_folder(None).unwrap();
        c.refresh().unwrap();
        let a = c
            .create_note(&json!({"title": "a", "folder": f, "rating": 4}).to_string())
            .unwrap();
        let before: Value = serde_json::from_str(&c.note_json(a).unwrap()).unwrap();
        assert_eq!(before["rating"], 0, "the folder does not rate");
        c.set_folder_rated(f, true).unwrap();
        c.refresh().unwrap();
        c.set_rating(a, 4).unwrap();
        let after: Value = serde_json::from_str(&c.note_json(a).unwrap()).unwrap();
        assert_eq!(after["rating"], 4);
        let b = make(&mut c, "b");
        c.set_favorite(b, true).unwrap();
        c.toggle_filter("favorite", 1);
        assert_eq!(page(&c)["count"], 1);
        assert_eq!(c.filter_count(), 1);
        c.toggle_filter("favorite", 0);
        c.toggle_filter("rating", 4);
        assert_eq!(titles(&page(&c)["groups"][0]["notes"]), ["a"]);
        c.clear_filters();
        assert_eq!(c.filter_count(), 0);
    }

    #[test]
    fn tags_filter_and_a_deleted_tag_stops_filtering() {
        let mut c = core();
        let a = make(&mut c, "a");
        make(&mut c, "b");
        c.set_tags(a, &["#work".to_string()]).unwrap();
        c.refresh().unwrap();
        c.toggle_tag_filter("work");
        assert_eq!(page(&c)["count"], 1);
        c.delete_tag("work", false).unwrap();
        c.refresh().unwrap();
        assert!(c.tag_filter.is_empty());
        assert_eq!(page(&c)["count"], 2);
    }

    #[test]
    fn search_finds_archived_notes_and_sorting_by_title_works() {
        let mut c = core();
        c.group_by_folder = false;
        let a = make(&mut c, "zebra");
        make(&mut c, "apple");
        c.set_archived(a, true).unwrap();
        assert_eq!(page(&c)["count"], 1);
        c.text = "zebra".into();
        assert_eq!(page(&c)["count"], 1);
        c.text.clear();
        c.set_scope("archive", -1);
        assert_eq!(titles(&page(&c)["notes"]), ["zebra"]);
        c.set_scope("notes", -1);
        c.set_sort("title");
        assert_eq!(c.sort_name(), "title");
    }

    #[test]
    fn folder_settings_edit_colour_icon_and_move() {
        let mut c = core();
        let a = c.create_folder(None).unwrap();
        let b = c.create_folder(None).unwrap();
        c.set_folder_title(a, "A").unwrap();
        c.set_folder_title(a, "  ").unwrap();
        c.set_folder_color(a, "#ff8000").unwrap();
        assert!(c.set_folder_color(a, "banana").is_err());
        c.set_folder_icon(a, "heart").unwrap();
        c.move_folder_into(b, Some(a)).unwrap();
        c.refresh().unwrap();
        let v: Value = serde_json::from_str(&c.folder_json(a).unwrap()).unwrap();
        assert_eq!(v["title"], "A");
        assert_eq!(v["color"], "#ff8000");
        assert_eq!(v["icon"], "emblem-favorite");
        assert_eq!(v["subfolders"], 1);
        let choices = v["parentChoices"].as_array().unwrap();
        assert!(choices.iter().all(|c| c["id"] != b && c["id"] != a));
        // A folder cannot move into the one below it.
        assert!(c.move_folder_into(a, Some(b)).is_err());
    }

    #[test]
    fn deleting_a_folder_keeps_or_removes_its_notes() {
        let mut c = core();
        let f = c.create_folder(None).unwrap();
        c.refresh().unwrap();
        c.create_note(&json!({"title": "n", "folder": f}).to_string())
            .unwrap();
        c.refresh().unwrap();
        let v: Value = serde_json::from_str(&c.folder_json(f).unwrap()).unwrap();
        assert_eq!(v["count"], 1);
        c.delete_folder(f, false).unwrap();
        c.refresh().unwrap();
        assert_eq!(page(&c)["count"], 1, "the note stays");
        let g = c.create_folder(None).unwrap();
        c.refresh().unwrap();
        c.create_note(&json!({"title": "m", "folder": g}).to_string())
            .unwrap();
        c.delete_folder(g, true).unwrap();
        c.refresh().unwrap();
        assert_eq!(page(&c)["count"], 1, "the other went with its folder");
        let default = c.default_folder().unwrap();
        assert!(c.delete_folder(default, false).is_err());
    }

    #[test]
    fn preferences_persist_and_change_the_page() {
        let d = temp("prefs");
        let (db, cfg) = (d.join("t.db"), d.join("c.ron"));
        let mut c = Core::open(&db, Some(PathBuf::new()), Some(cfg.clone())).unwrap();
        c.set_pref_bool("groupByFolder", false).unwrap();
        c.set_pref_bool("listView", true).unwrap();
        c.set_trash_days(7).unwrap();
        assert!(c.set_pref_bool("nonsense", true).is_err());
        let c = Core::open(&db, Some(PathBuf::new()), Some(cfg)).unwrap();
        assert!(!c.group_by_folder);
        let p: Value = serde_json::from_str(&c.prefs_json()).unwrap();
        assert_eq!(p["listView"], true);
        assert_eq!(p["trashDays"], 7);
    }

    fn md_files(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() && !p.ends_with(".trash") {
                out.extend(md_files(&p));
            } else if p.extension().is_some_and(|x| x == "md") {
                out.push(p);
            }
        }
        out
    }

    #[test]
    fn notes_reach_the_vault_and_files_outside_are_read_in() {
        let d = temp("vault");
        let vault = d.join("notes");
        let mut c =
            Core::open(&d.join("t.db"), Some(vault.clone()), Some(d.join("c.ron"))).unwrap();
        assert!(c.vault_dir().is_some());
        let id = c
            .create_note(&json!({"title": "water", "body": "the plants"}).to_string())
            .unwrap();
        c.flush().unwrap();
        let files = md_files(&vault);
        assert_eq!(files.len(), 1, "{files:?}");
        assert!(
            std::fs::read_to_string(&files[0])
                .unwrap()
                .contains("the plants")
        );
        c.edit_note(id, "water", "the ferns").unwrap();
        c.flush().unwrap();
        assert!(
            std::fs::read_to_string(&files[0])
                .unwrap()
                .contains("the ferns")
        );
        // A file added by another editor becomes a note.
        std::fs::write(vault.join("Outside.md"), "from elsewhere").unwrap();
        c.sync_vault().unwrap();
        c.refresh().unwrap();
        assert_eq!(page(&c)["count"], 2);
        // Trashing then purging a note removes its file; the trash is not a way to lose files.
        c.set_vault(None).unwrap();
        assert!(c.vault_dir().is_none());
    }

    #[test]
    fn importing_copies_a_file_into_the_vault() {
        let d = temp("import");
        let mut c = Core::open(
            &d.join("t.db"),
            Some(d.join("notes")),
            Some(d.join("c.ron")),
        )
        .unwrap();
        let src = d.join("elsewhere.md");
        std::fs::write(&src, "imported text").unwrap();
        assert_eq!(
            c.import(std::slice::from_ref(&src), false, None).unwrap(),
            1
        );
        assert!(src.exists(), "a copy leaves the original");
        c.refresh().unwrap();
        assert_eq!(page(&c)["count"], 1);
        let mut none = core();
        assert!(none.import(&[src], false, None).is_err());
    }
}
