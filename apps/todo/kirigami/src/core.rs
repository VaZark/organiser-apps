//! Todo's state and every action, with no Qt in it: the bridge in `backend.rs` only forwards.
//! Lists come out as JSON (an array of objects) that QML reads straight into a list model.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use organiser_core::settings::{Loaded, Settings, StartView};
use organiser_engine::{ChildPolicy, Db, Filters, Node, Result, TagFilter, build_tree};
use serde_json::{Value, json};
use todo_schema::lifecycle::{self, NewTask};
use todo_schema::priority::Priority;
use todo_schema::quadrant::{self, Quadrant};
use todo_schema::sort::{SortKey, sort_tasks};
use todo_schema::status::Statuses;
use todo_schema::statuses::{self, StatusEdit};
use todo_schema::task::{Edit, Task};
use todo_schema::vault::{Known, Vault};
use todo_schema::views::{self, Filter, Scope};

/// What the task list is split into: a header row, then that group's tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GroupBy {
    #[default]
    None,
    Status,
    /// Only when no single epic is open.
    Epic,
    /// Only inside one epic.
    Section,
}

impl GroupBy {
    pub fn parse(s: &str) -> Self {
        match s {
            "status" => Self::Status,
            "epic" => Self::Epic,
            "section" => Self::Section,
            _ => Self::None,
        }
    }
}

/// What the list shows, and the database behind it.
pub struct Core {
    db: Db,
    pub scope: Scope,
    pub epic: Option<i64>,
    pub no_epic: bool,
    pub show_closed: bool,
    pub show_subtasks: bool,
    pub text: String,
    /// Tasks must have all of these tags (or a tag below one).
    pub tag_filter: Vec<String>,
    pub sort: SortKey,
    pub group_by: GroupBy,
    /// The schema's own filters: status, priority, pinned, due.
    pub fields: Filters,
    statuses: Statuses,
    today: NaiveDate,
    db_path: PathBuf,
    /// The folder the tasks live in as Markdown files, when they do; every change is written to it
    /// (a task deleted for good has its file archived, never removed).
    vault: Option<Vault>,
    /// The file each task had after the last flush.
    known: Known,
    pub settings: Settings,
    /// What the person should be told after the last action (a vault warning, a settings problem).
    pub notice: Option<String>,
}

impl Core {
    /// Open the database, and the tasks folder the settings name (`vault` overrides it; an empty
    /// path means none), syncing the folder into the database.
    pub fn open(path: &Path, vault: Option<PathBuf>) -> Result<Self> {
        let mut db = Db::open(path, &todo_schema::SCHEMA)?;
        statuses::ensure_defaults(&mut db)?;
        let statuses = statuses::load(&db)?;
        let Loaded { settings, problem } = Settings::load();
        let mut core = Self {
            db,
            scope: Scope::All,
            epic: None,
            no_epic: false,
            show_closed: settings.todo.show_closed,
            show_subtasks: settings.todo.show_subtasks,
            text: String::new(),
            tag_filter: Vec::new(),
            sort: SortKey::default(),
            group_by: GroupBy::default(),
            fields: Filters::default(),
            statuses,
            today: views::today(),
            db_path: path.to_path_buf(),
            vault: None,
            known: Known::default(),
            settings,
            notice: problem,
        };
        let dir = match vault {
            Some(v) if v.as_os_str().is_empty() => None,
            Some(v) => Some(v),
            None => core.settings.todo.vault_dir.clone(),
        };
        if let Some(dir) = dir {
            core.open_vault(dir)?;
        }
        Ok(core)
    }

    /// Start keeping the tasks in this folder: read its files in, then write what the database
    /// has that they lack.
    fn open_vault(&mut self, dir: PathBuf) -> Result<()> {
        let v = Vault::open(dir, Some(&self.db_path))?;
        let mut report = v.sync(&mut self.db)?;
        report.add(v.flush(&mut self.db, &self.known)?);
        self.known = v.known(&self.db)?;
        self.notice = report
            .warnings
            .first()
            .cloned()
            .or_else(|| Some(report.summary_with("archived")));
        self.vault = Some(v);
        Ok(())
    }

    /// Choose the tasks folder (`None`: the database alone) and remember it in the settings.
    pub fn set_vault(&mut self, dir: Option<PathBuf>) -> Result<()> {
        self.vault = None;
        self.known = Known::default();
        self.settings.todo.vault_dir = dir.clone();
        if let Some(d) = dir
            && let Err(e) = self.open_vault(d)
        {
            self.settings.todo.vault_dir = None;
            let _ = self.settings.save();
            return Err(e);
        }
        let _ = self.settings.save();
        Ok(())
    }

    pub fn vault_dir(&self) -> Option<&Path> {
        self.vault.as_ref().map(Vault::dir)
    }

    /// Read the folder's files into the tasks (changes made outside the app), then write back.
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
                .unwrap_or_else(|| report.summary_with("archived")),
        );
        Ok(())
    }

    /// Write the database's changes to the folder. Called after every action.
    pub fn flush(&mut self) -> Result<()> {
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

    /// What Todo opens on: `notes`, `board`, `list` or `quadrants`.
    pub fn start_view(&self) -> &'static str {
        match self.settings.todo.start_view {
            StartView::Notes => "notes",
            StartView::Board => "board",
            StartView::List => "list",
            StartView::Quadrants => "quadrants",
        }
    }

    pub fn set_start_view(&mut self, v: &str) {
        self.settings.todo.start_view = match v {
            "board" => StartView::Board,
            "list" => StartView::List,
            "quadrants" => StartView::Quadrants,
            _ => StartView::Notes,
        };
    }

    /// Keep the lists' defaults (finished tasks, subtasks) for the next start.
    pub fn save_preferences(&mut self) -> std::result::Result<(), String> {
        self.settings.todo.show_closed = self.show_closed;
        self.settings.todo.show_subtasks = self.show_subtasks;
        self.settings.save()
    }

    fn filter(&self) -> Filter {
        Filter {
            scope: self.scope,
            show_closed: self.show_closed,
            text: (!self.text.trim().is_empty()).then(|| self.text.trim().to_string()),
            tags: TagFilter {
                all: self.tag_filter.clone(),
                ..TagFilter::default()
            },
            epic: self.epic,
            no_epic: self.no_epic,
            hide_subtasks: !self.show_subtasks,
            sort: self.sort,
            fields: self.fields.clone(),
            ..Filter::default()
        }
    }

    /// How many filters narrow the list: search, tags and the schema's own.
    pub fn filter_count(&self) -> usize {
        usize::from(!self.text.trim().is_empty()) + self.tag_filter.len() + self.fields.count()
    }

    /// What the filter sheet shows.
    pub fn filters_json(&self) -> String {
        let any = |f: &str| -> Vec<Value> { self.fields.any_of(f).to_vec() };
        json!({
            "count": self.filter_count(),
            "statuses": any("status"),
            "priorities": any("priority"),
            "pinned": !any("pinned").is_empty(),
            "overdue": self.fields.get("due").is_some(),
        })
        .to_string()
    }

    /// Toggle one of the schema's filters: `status` (an id), `priority` (a name), `pinned` or
    /// `overdue` (on or off).
    pub fn toggle_filter(&mut self, field: &str, value: &str) {
        match field {
            "status" => {
                if let Ok(id) = value.parse::<i64>() {
                    self.fields.toggle("status", json!(id));
                }
            }
            "priority" => self.fields.toggle("priority", json!(value)),
            "pinned" => {
                self.fields.remove("pinned");
                if value == "1" {
                    self.fields.toggle("pinned", Value::Bool(true));
                }
            }
            "overdue" => {
                let yesterday = self.today.pred_opt().map(|d| json!(d.to_string()));
                self.fields
                    .set_range("due", None, if value == "1" { yesterday } else { None });
            }
            _ => {}
        }
    }

    /// Search, tags and the schema's filters, all cleared.
    pub fn clear_filters(&mut self) {
        self.fields.clear();
        self.tag_filter.clear();
        self.text.clear();
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.today = views::today();
        self.statuses = statuses::load(&self.db)?;
        // Every tag has a colour of its own.
        let palette = self.settings.palette();
        self.db.ensure_tag_colors(&palette)?;
        // A tag that is gone no longer filters.
        let known: Vec<String> = self.db.tags()?.into_iter().map(|t| t.name).collect();
        self.tag_filter.retain(|t| known.contains(t));
        Ok(())
    }

    fn epic_titles(&self) -> HashMap<i64, String> {
        views::epics(&self.db)
            .map(|v| v.into_iter().map(|s| (s.epic.id, s.epic.title)).collect())
            .unwrap_or_default()
    }

    /// One task as the views draw it: its fields plus the status it is in, its epic's title, its
    /// nesting depth and the box of the Eisenhower matrix it belongs to.
    fn task_value(&self, t: &Task, depth: usize, epics: &HashMap<i64, String>) -> Value {
        let status = self.statuses.resolve(t.status);
        let mut v = serde_json::to_value(t).unwrap_or(Value::Null);
        if let Value::Object(m) = &mut v {
            m.insert("depth".into(), json!(depth));
            m.insert("statusId".into(), json!(status.map(|s| s.id)));
            m.insert("statusName".into(), json!(status.map(|s| s.name.clone())));
            m.insert(
                "statusColor".into(),
                json!(status.and_then(|s| s.color.clone())),
            );
            m.insert("closed".into(), json!(self.statuses.is_closed(t.status)));
            m.insert(
                "epicTitle".into(),
                json!(t.epic.and_then(|e| epics.get(&e).cloned())),
            );
            m.insert(
                "quadrant".into(),
                json!(match quadrant::classify(t.priority, t.due, self.today) {
                    Quadrant::DoFirst => 0,
                    Quadrant::Schedule => 1,
                    Quadrant::Delegate => 2,
                    Quadrant::Backlog => 3,
                }),
            );
            m.insert(
                "overdue".into(),
                json!(t.due.is_some_and(|d| d < self.today)),
            );
            m.insert("priorityLabel".into(), json!(t.priority.label()));
        }
        v
    }

    /// The grouping that makes sense for what is open: sections only inside an epic, epics only
    /// across epics. Anything else is shown ungrouped.
    pub fn effective_group(&self) -> GroupBy {
        match self.group_by {
            GroupBy::Section if self.epic.is_some() => GroupBy::Section,
            GroupBy::Epic if self.epic.is_none() && !self.no_epic => GroupBy::Epic,
            GroupBy::Status => GroupBy::Status,
            _ => GroupBy::None,
        }
    }

    /// The tasks as a nested list: each task, then its subtasks one level deeper. When grouped,
    /// a header row (`{"header": true, "title", "color", "count"}`) comes before each group's
    /// tasks; a subtask stays under its parent, so a group is a set of top-level tasks.
    pub fn tasks_json(&self) -> Result<String> {
        let tasks = views::tasks(&self.db, &self.filter(), self.today)?;
        let epics = self.epic_titles();
        // One unit per top-level task: it, then its subtree.
        let mut units: Vec<(Task, Vec<Value>)> = Vec::new();
        if self.scope == Scope::Current {
            for t in &tasks {
                units.push((t.clone(), vec![self.task_value(t, 0, &epics)]));
            }
        } else {
            fn walk(
                core: &Core,
                nodes: &[Node<Task>],
                d: usize,
                e: &HashMap<i64, String>,
                out: &mut Vec<Value>,
            ) {
                for n in nodes {
                    out.push(core.task_value(&n.value, d, e));
                    walk(core, &n.children, d + 1, e, out);
                }
            }
            let forest = build_tree(tasks, |t| t.id, |t| t.parent_id);
            for node in &forest {
                let mut rows = Vec::new();
                walk(self, std::slice::from_ref(node), 0, &epics, &mut rows);
                units.push((node.value.clone(), rows));
            }
        }
        let out = match self.groups() {
            Some((groups, key_of)) => {
                let mut out = Vec::new();
                let mut placed = vec![false; units.len()];
                let mut emit = |title: &str, color: Option<&str>, members: Vec<usize>| {
                    if members.is_empty() {
                        return;
                    }
                    let count = members.len();
                    out.push(
                        json!({"header": true, "title": title, "color": color, "count": count}),
                    );
                    for i in members {
                        out.extend(units[i].1.iter().cloned());
                    }
                };
                for (title, color, key) in &groups {
                    let members: Vec<usize> = (0..units.len())
                        .filter(|&i| !placed[i] && key_of(&units[i].0) == *key)
                        .collect();
                    members.iter().for_each(|&i| placed[i] = true);
                    emit(title, color.as_deref(), members);
                }
                // Whatever no group claimed (a task of an epic that is gone, say).
                let rest: Vec<usize> = (0..units.len()).filter(|&i| !placed[i]).collect();
                emit("Other", None, rest);
                out
            }
            None => units.into_iter().flat_map(|(_, rows)| rows).collect(),
        };
        Ok(Value::Array(out).to_string())
    }

    /// The groups to draw, in order as `(title, colour, key)`, and how to read a task's key; none
    /// when the list is not grouped. A key of `None` is the "no epic" / "no section" group.
    #[allow(clippy::type_complexity)]
    fn groups(
        &self,
    ) -> Option<(
        Vec<(String, Option<String>, Option<i64>)>,
        Box<dyn Fn(&Task) -> Option<i64> + '_>,
    )> {
        match self.effective_group() {
            GroupBy::None => None,
            GroupBy::Status => {
                let groups = self
                    .statuses
                    .all()
                    .iter()
                    .map(|s| (s.name.clone(), s.color.clone(), Some(s.id)))
                    .collect();
                Some((
                    groups,
                    Box::new(|t| self.statuses.resolve(t.status).map(|s| s.id)),
                ))
            }
            GroupBy::Epic => {
                let mut groups: Vec<_> = views::epics(&self.db)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|s| (s.epic.title, s.epic.color, Some(s.epic.id)))
                    .collect();
                groups.push(("No epic".into(), None, None));
                Some((groups, Box::new(|t| t.epic)))
            }
            GroupBy::Section => {
                let mut groups: Vec<_> = self
                    .epic
                    .and_then(|e| views::sections(&self.db, e).ok())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|s| (s.section.title, s.section.color, Some(s.section.id)))
                    .collect();
                groups.push(("No section".into(), None, None));
                Some((groups, Box::new(|t| t.section)))
            }
        }
    }

    /// Every task, closed ones too, ordered for the board and the matrix: pinned first, then by
    /// the chosen order.
    pub fn all_tasks_json(&self) -> Result<String> {
        let f = Filter {
            show_closed: true,
            hide_subtasks: !self.show_subtasks,
            ..self.filter()
        };
        let mut tasks = views::tasks(&self.db, &f, self.today)?;
        let mut subs: HashMap<i64, usize> = HashMap::new();
        for t in &tasks {
            if let Some(p) = t.parent_id {
                *subs.entry(p).or_default() += 1;
            }
        }
        sort_tasks(&mut tasks, self.sort, &subs);
        let epics = self.epic_titles();
        Ok(Value::Array(
            tasks
                .iter()
                .map(|t| self.task_value(t, 0, &epics))
                .collect(),
        )
        .to_string())
    }

    pub fn task_json(&self, id: i64) -> Result<String> {
        let t: Task = (&self.db.get(id)?).into();
        Ok(self.task_value(&t, 0, &self.epic_titles()).to_string())
    }

    pub fn epics_json(&self) -> Result<String> {
        let list: Vec<Value> = views::epics(&self.db)?
            .into_iter()
            .map(|s| {
                json!({
                    "id": s.epic.id, "title": s.epic.title, "notes": s.epic.notes,
                    "color": s.epic.color, "icon": s.epic.icon,
                    "open": s.open, "total": s.total, "progress": s.progress(),
                    "due": s.epic.due.map(|d| d.to_string()),
                })
            })
            .collect();
        Ok(Value::Array(list).to_string())
    }

    pub fn statuses_json(&self) -> String {
        serde_json::to_string(self.statuses.all()).unwrap_or_else(|_| "[]".into())
    }

    pub fn tags_json(&self) -> Result<String> {
        let tags: Vec<Value> = self
            .db
            .tags()?
            .into_iter()
            .map(|t| json!({"name": t.name, "color": t.color, "parent": t.parent}))
            .collect();
        Ok(Value::Array(tags).to_string())
    }

    /// `(unassigned open, unassigned total)`.
    pub fn unassigned(&self) -> (usize, usize) {
        views::unassigned(&self.db).unwrap_or((0, 0))
    }

    // ---- tasks -------------------------------------------------------------------------------

    pub fn create_task(
        &mut self,
        title: &str,
        epic: Option<i64>,
        parent: Option<i64>,
    ) -> Result<i64> {
        // Quick-add syntax: `#tag`, `@tag`, `due:` and `start:` words leave the title.
        let q = todo_schema::quick::parse(title, self.today);
        let mut n = NewTask::new(if q.title.is_empty() { title } else { &q.title });
        n.tags = q.tags;
        n.start = q.start;
        n.due = q.due;
        n.epic = epic.or(self.epic);
        n.parent = parent;
        Ok(lifecycle::create(&mut self.db, &n)?.id)
    }

    pub fn set_status(&mut self, id: i64, status: i64) -> Result<()> {
        lifecycle::set_status(&mut self.db, id, status)?;
        Ok(())
    }

    pub fn set_pinned(&mut self, id: i64, pinned: bool) -> Result<()> {
        lifecycle::set_pinned(&mut self.db, id, pinned)?;
        Ok(())
    }

    pub fn set_archived(&mut self, id: i64, archived: bool) -> Result<()> {
        lifecycle::set_archived(&mut self.db, id, archived)?;
        Ok(())
    }

    pub fn delete_task(&mut self, id: i64) -> Result<()> {
        self.db.delete(id)?;
        Ok(())
    }

    pub fn edit_title(&mut self, id: i64, title: &str) -> Result<()> {
        self.edit(
            id,
            Edit {
                title: Some(title.into()),
                ..Edit::default()
            },
        )
    }

    pub fn edit_notes(&mut self, id: i64, notes: &str) -> Result<()> {
        self.edit(
            id,
            Edit {
                notes: Some(notes.into()),
                ..Edit::default()
            },
        )
    }

    /// `due` is `YYYY-MM-DD`, or empty to clear it.
    pub fn edit_due(&mut self, id: i64, due: &str) -> Result<()> {
        let d = NaiveDate::parse_from_str(due.trim(), "%Y-%m-%d").ok();
        self.edit(
            id,
            Edit {
                due: Some(d),
                ..Edit::default()
            },
        )
    }

    pub fn edit_priority(&mut self, id: i64, priority: &str) -> Result<()> {
        let p = Priority::parse(priority).unwrap_or_default();
        self.edit(
            id,
            Edit {
                priority: Some(p),
                ..Edit::default()
            },
        )
    }

    pub fn edit_epic(&mut self, id: i64, epic: Option<i64>) -> Result<()> {
        self.edit(
            id,
            Edit {
                epic: Some(epic),
                ..Edit::default()
            },
        )
    }

    fn edit(&mut self, id: i64, e: Edit) -> Result<()> {
        lifecycle::edit(&mut self.db, id, &e)?;
        Ok(())
    }

    pub fn set_tags(&mut self, id: i64, tags: &[String]) -> Result<()> {
        lifecycle::set_tags(&mut self.db, id, tags)
    }

    // ---- tags --------------------------------------------------------------------------------

    pub fn create_tag(&mut self, name: &str, parent: &str) -> Result<()> {
        self.db
            .create_tag(name, (!parent.is_empty()).then_some(parent))?;
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

    // ---- epics and statuses ------------------------------------------------------------------

    pub fn create_epic(&mut self, title: &str) -> Result<i64> {
        Ok(lifecycle::create_epic(&mut self.db, title)?.id)
    }

    /// One epic with its counts, for its settings page.
    pub fn epic_json(&self, id: i64) -> Result<String> {
        let all: Vec<Value> = serde_json::from_str(&self.epics_json()?)?;
        Ok(all
            .into_iter()
            .find(|e| e["id"] == id)
            .unwrap_or_else(|| json!({}))
            .to_string())
    }

    fn edit_epic_with(&mut self, id: i64, e: Edit) -> Result<()> {
        lifecycle::edit_epic(&mut self.db, id, &e)?;
        Ok(())
    }

    pub fn set_epic_title(&mut self, id: i64, title: &str) -> Result<()> {
        self.edit_epic_with(
            id,
            Edit {
                title: Some(title.into()),
                ..Edit::default()
            },
        )
    }

    pub fn set_epic_notes(&mut self, id: i64, notes: &str) -> Result<()> {
        self.edit_epic_with(
            id,
            Edit {
                notes: Some(notes.into()),
                ..Edit::default()
            },
        )
    }

    /// `YYYY-MM-DD`, or anything else to clear it.
    pub fn set_epic_due(&mut self, id: i64, due: &str) -> Result<()> {
        let d = NaiveDate::parse_from_str(due.trim(), "%Y-%m-%d").ok();
        self.edit_epic_with(
            id,
            Edit {
                due: Some(d),
                ..Edit::default()
            },
        )
    }

    /// `#rrggbb`, or empty for none.
    pub fn set_epic_color(&mut self, id: i64, color: &str) -> Result<()> {
        let ok = color.len() == 7
            && color.starts_with('#')
            && color[1..].chars().all(|c| c.is_ascii_hexdigit());
        self.edit_epic_with(
            id,
            Edit {
                color: Some(ok.then(|| color.to_string())),
                ..Edit::default()
            },
        )
    }

    /// An icon name, or empty for the folder.
    pub fn set_epic_icon(&mut self, id: i64, icon: &str) -> Result<()> {
        self.edit_epic_with(
            id,
            Edit {
                icon: Some((!icon.is_empty()).then(|| icon.to_string())),
                ..Edit::default()
            },
        )
    }

    pub fn delete_epic(&mut self, id: i64, with_tasks: bool) -> Result<()> {
        if with_tasks {
            lifecycle::delete_epic_and_tasks(&mut self.db, id)?;
        } else {
            lifecycle::delete_epic(&mut self.db, id)?;
        }
        if self.epic == Some(id) {
            self.epic = None;
        }
        Ok(())
    }

    pub fn create_status(&mut self, name: &str, color: &str, closed: bool) -> Result<()> {
        statuses::create(
            &mut self.db,
            name,
            (!color.is_empty()).then_some(color),
            closed,
        )?;
        Ok(())
    }

    pub fn edit_status(&mut self, id: i64, name: &str, color: &str, closed: bool) -> Result<()> {
        statuses::edit(
            &mut self.db,
            id,
            &StatusEdit {
                name: Some(name.into()),
                color: Some((!color.is_empty()).then(|| color.to_string())),
                closed: Some(closed),
            },
        )?;
        Ok(())
    }

    pub fn move_status(&mut self, id: i64, step: i32) -> Result<()> {
        statuses::move_by(&mut self.db, id, step)
    }

    pub fn delete_status(&mut self, id: i64) -> Result<()> {
        statuses::delete(&mut self.db, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "todo-kirigami-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn core() -> Core {
        // An empty vault path: none, whatever the person's settings say.
        Core::open(&temp("db").join("t.db"), Some(PathBuf::new())).unwrap()
    }

    fn parse(s: String) -> Vec<Value> {
        serde_json::from_str::<Value>(&s)
            .unwrap()
            .as_array()
            .unwrap()
            .clone()
    }

    #[test]
    fn an_epic_keeps_its_settings() {
        let mut c = core();
        let e = c.create_epic("Garden").unwrap();
        c.set_epic_title(e, "Garden 2").unwrap();
        c.set_epic_notes(e, "spring work").unwrap();
        c.set_epic_due(e, "2030-05-01").unwrap();
        c.set_epic_color(e, "#33aa66").unwrap();
        c.set_epic_icon(e, "folder-green").unwrap();
        let v: Value = serde_json::from_str(&c.epic_json(e).unwrap()).unwrap();
        assert_eq!(v["title"], "Garden 2");
        assert_eq!(v["notes"], "spring work");
        assert_eq!(v["due"], "2030-05-01");
        assert_eq!(v["color"], "#33aa66");
        assert_eq!(v["icon"], "folder-green");
        // A bad colour clears it; an empty icon goes back to the folder.
        c.set_epic_color(e, "green").unwrap();
        c.set_epic_icon(e, "").unwrap();
        let v: Value = serde_json::from_str(&c.epic_json(e).unwrap()).unwrap();
        assert!(v["color"].is_null() && v["icon"].is_null());
    }

    #[test]
    fn quick_add_words_become_tags_and_dates() {
        let mut c = core();
        let id = c
            .create_task("Buy milk #home due:2030-01-02", None, None)
            .unwrap();
        let t = c.task_json(id).unwrap();
        let v: Value = serde_json::from_str(&t).unwrap();
        assert_eq!(v["title"], "Buy milk");
        assert_eq!(v["due"], "2030-01-02");
        assert_eq!(v["tags"][0], "home");
    }

    #[test]
    fn a_new_task_lists_with_its_status_and_closing_hides_it() {
        let mut c = core();
        let id = c.create_task("write the port", None, None).unwrap();
        let tasks = parse(c.tasks_json().unwrap());
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0]["title"], "write the port");
        assert_eq!(tasks[0]["statusName"], "To do");
        assert_eq!(tasks[0]["closed"], false);
        let done = c.statuses.first_closed().unwrap().id;
        c.set_status(id, done).unwrap();
        c.refresh().unwrap();
        assert!(parse(c.tasks_json().unwrap()).is_empty());
        c.show_closed = true;
        assert_eq!(parse(c.tasks_json().unwrap())[0]["closed"], true);
    }

    #[test]
    fn subtasks_nest_one_level_deeper() {
        let mut c = core();
        let p = c.create_task("parent", None, None).unwrap();
        c.create_task("child", None, Some(p)).unwrap();
        let tasks = parse(c.tasks_json().unwrap());
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0]["depth"], 0);
        assert_eq!(tasks[1]["depth"], 1);
    }

    #[test]
    fn tags_filter_and_a_deleted_tag_stops_filtering() {
        let mut c = core();
        let a = c.create_task("a", None, None).unwrap();
        c.create_task("b", None, None).unwrap();
        c.set_tags(a, &["work".to_string()]).unwrap();
        c.tag_filter = vec!["work".into()];
        let tasks = parse(c.tasks_json().unwrap());
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0]["title"], "a");
        c.delete_tag("work", false).unwrap();
        c.refresh().unwrap();
        assert!(c.tag_filter.is_empty());
        assert_eq!(parse(c.tasks_json().unwrap()).len(), 2);
    }

    /// Titles of the headers, and the titles under each, as `"Header: a, b"` lines.
    fn outline(c: &Core) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for v in parse(c.tasks_json().unwrap()) {
            if v["header"] == true {
                out.push(format!("{}:", v["title"].as_str().unwrap()));
            } else {
                let last = out.last_mut().unwrap();
                last.push_str(&format!(" {}", v["title"].as_str().unwrap()));
            }
        }
        out
    }

    #[test]
    fn grouping_by_status_follows_the_status_order_and_keeps_subtasks_with_parents() {
        let mut c = core();
        let a = c.create_task("a", None, None).unwrap();
        let b = c.create_task("b", None, None).unwrap();
        c.create_task("a-child", None, Some(a)).unwrap();
        let doing = c.statuses.by_name("In progress").unwrap().id;
        c.set_status(b, doing).unwrap();
        c.refresh().unwrap();
        c.group_by = GroupBy::Status;
        assert_eq!(outline(&c), ["To do: a a-child", "In progress: b"]);
        let rows = parse(c.tasks_json().unwrap());
        assert_eq!(rows[0]["count"], 1);
    }

    #[test]
    fn grouping_by_epic_lists_epics_in_order_then_no_epic() {
        let mut c = core();
        let e1 = c.create_epic("Home").unwrap();
        let e2 = c.create_epic("Work").unwrap();
        c.create_task("loose", None, None).unwrap();
        c.create_task("w", Some(e2), None).unwrap();
        c.create_task("h", Some(e1), None).unwrap();
        c.group_by = GroupBy::Epic;
        assert_eq!(outline(&c), ["Home: h", "Work: w", "No epic: loose"]);
        // One epic open, or only the unassigned: epics are no grouping.
        c.epic = Some(e1);
        assert_eq!(c.effective_group(), GroupBy::None);
        c.epic = None;
        c.no_epic = true;
        assert_eq!(c.effective_group(), GroupBy::None);
    }

    #[test]
    fn grouping_by_section_needs_an_epic_and_ends_with_no_section() {
        let mut c = core();
        let e = c.create_epic("Garden").unwrap();
        let s1 = lifecycle::create_section(&mut c.db, e, "Spring").unwrap();
        lifecycle::create_section(&mut c.db, e, "Empty").unwrap();
        let t = c.create_task("sow", Some(e), None).unwrap();
        c.create_task("plan", Some(e), None).unwrap();
        c.edit(
            t,
            Edit {
                section: Some(Some(s1.id)),
                ..Edit::default()
            },
        )
        .unwrap();
        c.group_by = GroupBy::Section;
        assert_eq!(c.effective_group(), GroupBy::None, "no epic open");
        c.epic = Some(e);
        assert_eq!(outline(&c), ["Spring: sow", "No section: plan"]);
    }

    #[test]
    fn status_priority_pinned_and_overdue_filters_narrow_the_list() {
        let mut c = core();
        let a = c.create_task("a", None, None).unwrap();
        let b = c.create_task("b due:2000-01-01", None, None).unwrap();
        c.set_pinned(a, true).unwrap();
        c.edit_priority(b, "high").unwrap();
        let doing = c.statuses.by_name("In progress").unwrap().id;
        c.set_status(b, doing).unwrap();
        let titles = |c: &Core| -> Vec<String> {
            parse(c.tasks_json().unwrap())
                .iter()
                .map(|v| v["title"].as_str().unwrap().to_string())
                .collect()
        };
        assert_eq!(c.filter_count(), 0);
        c.toggle_filter("status", &doing.to_string());
        assert_eq!(titles(&c), ["b"]);
        c.toggle_filter("status", &doing.to_string());
        c.toggle_filter("priority", "high");
        assert_eq!(titles(&c), ["b"]);
        c.clear_filters();
        c.toggle_filter("pinned", "1");
        assert_eq!(titles(&c), ["a"]);
        c.toggle_filter("pinned", "0");
        c.toggle_filter("overdue", "1");
        assert_eq!(titles(&c), ["b"]);
        assert_eq!(c.filter_count(), 1);
        c.text = "zzz".into();
        c.tag_filter = vec!["x".into()];
        assert_eq!(c.filter_count(), 3);
        c.clear_filters();
        assert_eq!(c.filter_count(), 0);
        assert_eq!(titles(&c).len(), 2);
    }

    #[test]
    fn the_start_view_names_round_trip() {
        let mut c = core();
        for v in ["notes", "board", "list", "quadrants"] {
            c.set_start_view(v);
            assert_eq!(c.start_view(), v);
        }
        c.set_start_view("nonsense");
        assert_eq!(c.start_view(), "notes");
    }

    #[test]
    fn edits_and_epics_round_trip() {
        let mut c = core();
        let e = c.create_epic("Home").unwrap();
        let id = c.create_task("paint", Some(e), None).unwrap();
        c.edit_priority(id, "highest").unwrap();
        c.edit_due(id, "2020-01-01").unwrap();
        let t: Value = serde_json::from_str(&c.task_json(id).unwrap()).unwrap();
        assert_eq!(t["epicTitle"], "Home");
        assert_eq!(t["overdue"], true);
        assert_eq!(t["quadrant"], 0);
        let epics = parse(c.epics_json().unwrap());
        assert_eq!(epics[0]["total"], 1);
    }

    fn md_files(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() && !p.ends_with(".archive") {
                out.extend(md_files(&p));
            } else if p.extension().is_some_and(|x| x == "md") {
                out.push(p);
            }
        }
        out
    }

    #[test]
    fn changes_reach_the_vault_archive_keeps_the_file_and_delete_removes_it() {
        let dir = temp("vault");
        let mut c = Core::open(&dir.join("t.db"), Some(dir.join("tasks"))).unwrap();
        assert!(c.vault_dir().is_some());
        let id = c.create_task("water the plants", None, None).unwrap();
        c.flush().unwrap();
        let files = md_files(&dir.join("tasks"));
        assert_eq!(files.len(), 1, "{files:?}");
        assert!(
            std::fs::read_to_string(&files[0])
                .unwrap()
                .contains("water the plants")
        );
        c.edit_title(id, "water the ferns").unwrap();
        c.flush().unwrap();
        assert!(
            std::fs::read_to_string(&md_files(&dir.join("tasks"))[0])
                .unwrap()
                .contains("water the ferns")
        );
        // Archiving keeps the file, under `.archive/`.
        c.set_archived(id, true).unwrap();
        c.flush().unwrap();
        assert!(md_files(&dir.join("tasks")).is_empty());
        assert_eq!(md_files(&dir.join("tasks/.archive")).len(), 1);
        // Deleting for good (the person confirmed) removes the task's file, as the iced app does.
        c.delete_task(id).unwrap();
        c.flush().unwrap();
        assert!(md_files(&dir.join("tasks/.archive")).is_empty());
    }
}
