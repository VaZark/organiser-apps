//! The QML-facing object: a thin shell over [`Core`]. Every invokable runs a core action, then
//! re-publishes the lists; a failure becomes the `toast` text.

use core::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::core::Core;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, tasks)]
        #[qproperty(QString, all_tasks, cxx_name = "allTasks")]
        #[qproperty(QString, epics)]
        #[qproperty(QString, statuses)]
        #[qproperty(QString, tags)]
        #[qproperty(QString, toast)]
        #[qproperty(i32, unassigned_open, cxx_name = "unassignedOpen")]
        #[qproperty(i32, unassigned_total, cxx_name = "unassignedTotal")]
        #[qproperty(QString, scope)]
        #[qproperty(i64, epic)]
        #[qproperty(bool, no_epic, cxx_name = "noEpic")]
        #[qproperty(bool, show_closed, cxx_name = "showClosed")]
        #[qproperty(bool, show_subtasks, cxx_name = "showSubtasks")]
        #[qproperty(QString, vault_dir, cxx_name = "vaultDir")]
        #[qproperty(QString, tag_filter, cxx_name = "tagFilter")]
        #[qproperty(QString, group_by, cxx_name = "groupBy")]
        #[qproperty(QString, filters)]
        #[qproperty(QString, start_view, cxx_name = "startView")]
        #[qproperty(QString, search)]
        #[qproperty(QString, sort)]
        type Backend = super::BackendRust;

        #[qinvokable]
        fn init(self: Pin<&mut Backend>);
        #[qinvokable]
        fn reload(self: Pin<&mut Backend>);
        /// Re-read after `scope`, `epic`, `no_epic`, `show_closed`, `show_subtasks`, `search`,
        /// `sort` or `group_by` changed.
        #[qinvokable]
        #[cxx_name = "applyFilters"]
        fn apply_filters(self: Pin<&mut Backend>);
        /// Toggle a filter of the filter sheet: `status` (id), `priority` (name), `pinned`,
        /// `overdue` (`1`/`0`).
        #[qinvokable]
        #[cxx_name = "toggleFilter"]
        fn toggle_filter(self: Pin<&mut Backend>, field: &QString, value: &QString);
        /// Clear search, tags and every other filter.
        #[qinvokable]
        #[cxx_name = "clearFilters"]
        fn clear_filters(self: Pin<&mut Backend>);
        /// Choose the folder the tasks are kept in (empty: none).
        #[qinvokable]
        #[cxx_name = "setVault"]
        fn set_vault(self: Pin<&mut Backend>, dir: &QString);
        /// Read changes made to the folder's files, then write ours.
        #[qinvokable]
        #[cxx_name = "syncVault"]
        fn sync_vault(self: Pin<&mut Backend>);
        /// Keep the lists' defaults for the next start.
        #[qinvokable]
        #[cxx_name = "savePreferences"]
        fn save_preferences(self: Pin<&mut Backend>);
        #[qinvokable]
        fn task(self: &Backend, id: i64) -> QString;
        /// Put a task under another (`parent` -1: out to the top level).
        #[qinvokable]
        #[cxx_name = "setParent"]
        fn set_parent(self: Pin<&mut Backend>, id: i64, parent: i64);
        /// The tasks a task may go under, as JSON.
        #[qinvokable]
        #[cxx_name = "parentChoices"]
        fn parent_choices(self: &Backend, id: i64) -> QString;
        /// The direct subtasks of a task as JSON, closed ones too.
        #[qinvokable]
        fn subtasks(self: &Backend, id: i64) -> QString;
        /// The value after `--name` on the command line, or empty (development flags).
        #[qinvokable]
        fn arg(self: &Backend, name: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "createTask"]
        fn create_task(self: Pin<&mut Backend>, title: &QString, epic: i64, parent: i64) -> i64;
        #[qinvokable]
        #[cxx_name = "setStatus"]
        fn set_status(self: Pin<&mut Backend>, id: i64, status: i64);
        #[qinvokable]
        #[cxx_name = "setPinned"]
        fn set_pinned(self: Pin<&mut Backend>, id: i64, pinned: bool);
        #[qinvokable]
        #[cxx_name = "setArchived"]
        fn set_archived(self: Pin<&mut Backend>, id: i64, archived: bool);
        #[qinvokable]
        #[cxx_name = "deleteTask"]
        fn delete_task(self: Pin<&mut Backend>, id: i64);
        #[qinvokable]
        #[cxx_name = "editTitle"]
        fn edit_title(self: Pin<&mut Backend>, id: i64, title: &QString);
        #[qinvokable]
        #[cxx_name = "editNotes"]
        fn edit_notes(self: Pin<&mut Backend>, id: i64, notes: &QString);
        #[qinvokable]
        #[cxx_name = "setTaskColor"]
        fn set_task_color(self: Pin<&mut Backend>, id: i64, color: &QString);
        /// Make a task a desktop sticky (or not).
        #[qinvokable]
        #[cxx_name = "setSticky"]
        fn set_sticky(self: Pin<&mut Backend>, id: i64, on: bool);
        /// Whether Sticky Notes shows this app's stickies, and the switch for it.
        #[qinvokable]
        #[cxx_name = "stickyLinked"]
        fn sticky_linked(self: &Backend) -> bool;
        #[qinvokable]
        #[cxx_name = "setStickyLinked"]
        fn set_sticky_linked(self: Pin<&mut Backend>, on: bool);
        /// The quick-pick colours as JSON (`[{"name", "hex"}]`).
        #[qinvokable]
        #[cxx_name = "colorPresets"]
        fn color_presets(self: &Backend) -> QString;
        #[qinvokable]
        #[cxx_name = "editDue"]
        fn edit_due(self: Pin<&mut Backend>, id: i64, due: &QString);
        #[qinvokable]
        #[cxx_name = "editPriority"]
        fn edit_priority(self: Pin<&mut Backend>, id: i64, priority: &QString);
        #[qinvokable]
        #[cxx_name = "editEpic"]
        fn edit_epic(self: Pin<&mut Backend>, id: i64, epic: i64);
        #[qinvokable]
        #[cxx_name = "setTaskTags"]
        fn set_task_tags(self: Pin<&mut Backend>, id: i64, tags: &QString);

        #[qinvokable]
        #[cxx_name = "createTag"]
        fn create_tag(self: Pin<&mut Backend>, name: &QString, parent: &QString);
        #[qinvokable]
        #[cxx_name = "setTagColor"]
        fn set_tag_color(self: Pin<&mut Backend>, name: &QString, color: &QString);
        #[qinvokable]
        #[cxx_name = "setTagParent"]
        fn set_tag_parent(self: Pin<&mut Backend>, name: &QString, parent: &QString);
        #[qinvokable]
        #[cxx_name = "deleteTag"]
        fn delete_tag(self: Pin<&mut Backend>, name: &QString, with_children: bool);
        #[qinvokable]
        #[cxx_name = "createEpic"]
        fn create_epic(self: Pin<&mut Backend>, title: &QString) -> i64;
        #[qinvokable]
        #[cxx_name = "epicInfo"]
        fn epic_info(self: &Backend, id: i64) -> QString;
        #[qinvokable]
        #[cxx_name = "setEpicTitle"]
        fn set_epic_title(self: Pin<&mut Backend>, id: i64, title: &QString);
        #[qinvokable]
        #[cxx_name = "setEpicNotes"]
        fn set_epic_notes(self: Pin<&mut Backend>, id: i64, notes: &QString);
        #[qinvokable]
        #[cxx_name = "setEpicDue"]
        fn set_epic_due(self: Pin<&mut Backend>, id: i64, due: &QString);
        #[qinvokable]
        #[cxx_name = "setEpicColor"]
        fn set_epic_color(self: Pin<&mut Backend>, id: i64, color: &QString);
        #[qinvokable]
        #[cxx_name = "setEpicIcon"]
        fn set_epic_icon(self: Pin<&mut Backend>, id: i64, icon: &QString);
        #[qinvokable]
        #[cxx_name = "deleteEpic"]
        fn delete_epic(self: Pin<&mut Backend>, id: i64, with_tasks: bool);
        #[qinvokable]
        #[cxx_name = "createStatus"]
        fn create_status(self: Pin<&mut Backend>, name: &QString, color: &QString, closed: bool);
        #[qinvokable]
        #[cxx_name = "editStatus"]
        fn edit_status(
            self: Pin<&mut Backend>,
            id: i64,
            name: &QString,
            color: &QString,
            closed: bool,
        );
        #[qinvokable]
        #[cxx_name = "moveStatus"]
        fn move_status(self: Pin<&mut Backend>, id: i64, step: i32);
        #[qinvokable]
        #[cxx_name = "deleteStatus"]
        fn delete_status(self: Pin<&mut Backend>, id: i64);
    }
}

pub struct BackendRust {
    core: Option<Core>,
    tasks: QString,
    all_tasks: QString,
    epics: QString,
    statuses: QString,
    tags: QString,
    toast: QString,
    unassigned_open: i32,
    unassigned_total: i32,
    scope: QString,
    epic: i64,
    no_epic: bool,
    show_closed: bool,
    show_subtasks: bool,
    vault_dir: QString,
    tag_filter: QString,
    group_by: QString,
    filters: QString,
    start_view: QString,
    search: QString,
    sort: QString,
}

impl Default for BackendRust {
    fn default() -> Self {
        Self {
            core: None,
            tasks: QString::from("[]"),
            all_tasks: QString::from("[]"),
            epics: QString::from("[]"),
            statuses: QString::from("[]"),
            tags: QString::from("[]"),
            toast: QString::default(),
            unassigned_open: 0,
            unassigned_total: 0,
            scope: QString::from("all"),
            epic: -1,
            no_epic: false,
            show_closed: false,
            show_subtasks: true,
            vault_dir: QString::default(),
            tag_filter: QString::default(),
            group_by: QString::default(),
            filters: QString::from("{}"),
            start_view: QString::from("notes"),
            search: QString::default(),
            sort: QString::from("manual"),
        }
    }
}

fn opt(id: i64) -> Option<i64> {
    (id >= 0).then_some(id)
}

fn sort_key(s: &str) -> todo_schema::sort::SortKey {
    use todo_schema::sort::SortKey;
    match s {
        "priority" => SortKey::Priority,
        "created" => SortKey::Created,
        "due" => SortKey::Due,
        _ => SortKey::Manual,
    }
}

fn scope_of(s: &str) -> todo_schema::views::Scope {
    use todo_schema::views::Scope;
    match s {
        "current" => Scope::Current,
        "archived" => Scope::Archived,
        _ => Scope::All,
    }
}

impl qobject::Backend {
    fn db_path() -> std::path::PathBuf {
        let args: Vec<String> = std::env::args().collect();
        args.iter()
            .position(|a| a == "--db")
            .and_then(|i| args.get(i + 1))
            .map_or_else(
                || organiser_core::paths::db_path(&todo_schema::META),
                std::path::PathBuf::from,
            )
    }

    pub fn init(mut self: Pin<&mut Self>) {
        // Tell Sticky Notes this app exists (switched off until the person links it).
        if let Err(e) = organiser_sticky::store::publish(&todo_schema::STICKY) {
            eprintln!("sticky registry: {e}");
        }
        // `--vault DIR` picks a tasks folder for a trial; `--vault ''` means none.
        let vault = std::env::args()
            .skip_while(|a| a != "--vault")
            .nth(1)
            .map(std::path::PathBuf::from);
        match Core::open(&Self::db_path(), vault) {
            Ok(core) => {
                let (closed, subs) = (core.show_closed, core.show_subtasks);
                let start = core.start_view();
                self.as_mut().rust_mut().core = Some(core);
                self.as_mut().set_start_view(QString::from(start));
                self.as_mut().set_show_closed(closed);
                self.as_mut().set_show_subtasks(subs);
                self.reload();
            }
            Err(e) => self.set_toast(QString::from(format!("cannot open the database: {e}"))),
        }
    }

    /// Run `f` on the core, then re-publish; an error is shown as the toast.
    fn act(mut self: Pin<&mut Self>, f: impl FnOnce(&mut Core) -> organiser_engine::Result<()>) {
        let result = match self.as_mut().rust_mut().core.as_mut() {
            Some(core) => f(core).and_then(|()| core.flush()),
            None => return,
        };
        if let Err(e) = result {
            self.as_mut().set_toast(QString::from(e.to_string()));
        }
        self.reload();
    }

    pub fn reload(mut self: Pin<&mut Self>) {
        let published = {
            let mut rust = self.as_mut().rust_mut();
            let Some(core) = rust.core.as_mut() else {
                return;
            };
            let _ = core.refresh();
            let filter = core.tag_filter.join(",");
            let filters = core.filters_json();
            let notice = core.notice.take();
            let vault = core
                .vault_dir()
                .map(|d| d.display().to_string())
                .unwrap_or_default();
            (
                filter,
                filters,
                notice,
                vault,
                core.tasks_json().unwrap_or_else(|e| e.to_string()),
                core.all_tasks_json().unwrap_or_else(|_| "[]".into()),
                core.epics_json().unwrap_or_else(|_| "[]".into()),
                core.statuses_json(),
                core.tags_json().unwrap_or_else(|_| "[]".into()),
                core.unassigned(),
            )
        };
        let (filter, filters, notice, vault, tasks, all, epics, statuses, tags, (uo, ut)) =
            published;
        if let Some(n) = notice {
            self.as_mut().set_toast(QString::from(n));
        }
        self.as_mut().set_vault_dir(QString::from(vault));
        self.as_mut().set_tag_filter(QString::from(filter));
        self.as_mut().set_filters(QString::from(filters));
        self.as_mut().set_tasks(QString::from(tasks));
        self.as_mut().set_all_tasks(QString::from(all));
        self.as_mut().set_epics(QString::from(epics));
        self.as_mut().set_statuses(QString::from(statuses));
        self.as_mut().set_tags(QString::from(tags));
        self.as_mut().set_unassigned_open(uo as i32);
        self.as_mut().set_unassigned_total(ut as i32);
    }

    pub fn toggle_filter(self: Pin<&mut Self>, field: &QString, value: &QString) {
        let (field, value) = (field.to_string(), value.to_string());
        self.act(|c| {
            c.toggle_filter(&field, &value);
            Ok(())
        });
    }

    pub fn clear_filters(mut self: Pin<&mut Self>) {
        self.as_mut().set_search(QString::default());
        self.as_mut().set_tag_filter(QString::default());
        if let Some(core) = self.as_mut().rust_mut().core.as_mut() {
            core.clear_filters();
        }
        self.apply_filters();
    }

    pub fn set_vault(self: Pin<&mut Self>, dir: &QString) {
        let dir = dir.to_string();
        let dir = dir.strip_prefix("file://").unwrap_or(&dir);
        let dir = (!dir.is_empty()).then(|| std::path::PathBuf::from(dir));
        self.act(|c| c.set_vault(dir));
    }

    pub fn sync_vault(self: Pin<&mut Self>) {
        self.act(|c| c.sync_vault());
    }

    pub fn save_preferences(mut self: Pin<&mut Self>) {
        let (closed, subs, start) = (
            *self.show_closed(),
            *self.show_subtasks(),
            self.start_view().to_string(),
        );
        let result = match self.as_mut().rust_mut().core.as_mut() {
            Some(core) => {
                core.show_closed = closed;
                core.show_subtasks = subs;
                core.set_start_view(&start);
                core.save_preferences()
            }
            None => return,
        };
        if let Err(e) = result {
            self.set_toast(QString::from(e));
        }
    }

    pub fn apply_filters(mut self: Pin<&mut Self>) {
        let (scope, epic, no_epic, closed, subs, search, sort, tag_filter, group) = (
            self.scope().to_string(),
            *self.epic(),
            *self.no_epic(),
            *self.show_closed(),
            *self.show_subtasks(),
            self.search().to_string(),
            self.sort().to_string(),
            self.tag_filter().to_string(),
            self.group_by().to_string(),
        );
        if let Some(core) = self.as_mut().rust_mut().core.as_mut() {
            core.group_by = crate::core::GroupBy::parse(&group);
            core.scope = scope_of(&scope);
            core.epic = opt(epic);
            core.no_epic = no_epic;
            core.show_closed = closed;
            core.show_subtasks = subs;
            core.text = search;
            core.sort = sort_key(&sort);
            core.tag_filter = tag_filter
                .split(',')
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(String::from)
                .collect();
        }
        self.reload();
    }

    pub fn arg(&self, name: &QString) -> QString {
        let flag = format!("--{name}");
        let args: Vec<String> = std::env::args().collect();
        args.iter()
            .position(|a| *a == flag)
            .and_then(|i| args.get(i + 1))
            .map_or_else(QString::default, |v| QString::from(v.as_str()))
    }

    pub fn task(&self, id: i64) -> QString {
        self.core
            .as_ref()
            .and_then(|c| c.task_json(id).ok())
            .map_or_else(|| QString::from("{}"), QString::from)
    }

    pub fn set_parent(self: Pin<&mut Self>, id: i64, parent: i64) {
        self.act(|c| c.set_parent(id, (parent >= 0).then_some(parent)));
    }

    pub fn parent_choices(&self, id: i64) -> QString {
        self.core
            .as_ref()
            .and_then(|c| c.parent_choices_json(id).ok())
            .map_or_else(|| QString::from("[]"), QString::from)
    }

    pub fn subtasks(&self, id: i64) -> QString {
        self.core
            .as_ref()
            .and_then(|c| c.subtasks_json(id).ok())
            .map_or_else(|| QString::from("[]"), QString::from)
    }

    pub fn create_task(mut self: Pin<&mut Self>, title: &QString, epic: i64, parent: i64) -> i64 {
        let title = title.to_string();
        let mut id = -1;
        self.as_mut().act(|c| {
            id = c.create_task(&title, opt(epic), opt(parent))?;
            Ok(())
        });
        id
    }

    pub fn set_status(self: Pin<&mut Self>, id: i64, status: i64) {
        self.act(|c| c.set_status(id, status));
    }

    pub fn set_pinned(self: Pin<&mut Self>, id: i64, pinned: bool) {
        self.act(|c| c.set_pinned(id, pinned));
    }

    pub fn set_archived(self: Pin<&mut Self>, id: i64, archived: bool) {
        self.act(|c| c.set_archived(id, archived));
    }

    pub fn delete_task(self: Pin<&mut Self>, id: i64) {
        self.act(|c| c.delete_task(id));
    }

    pub fn edit_title(self: Pin<&mut Self>, id: i64, title: &QString) {
        let v = title.to_string();
        self.act(|c| c.edit_title(id, &v));
    }

    pub fn edit_notes(self: Pin<&mut Self>, id: i64, notes: &QString) {
        let v = notes.to_string();
        self.act(|c| c.edit_notes(id, &v));
    }

    pub fn set_task_color(self: Pin<&mut Self>, id: i64, color: &QString) {
        let v = color.to_string();
        self.act(|c| c.set_task_color(id, &v));
    }

    pub fn set_sticky(self: Pin<&mut Self>, id: i64, on: bool) {
        self.act(|c| c.set_sticky(id, on));
    }

    pub fn sticky_linked(&self) -> bool {
        organiser_sticky::store::is_linked(&todo_schema::STICKY)
    }

    pub fn set_sticky_linked(mut self: Pin<&mut Self>, on: bool) {
        if let Err(e) = organiser_sticky::store::set_linked(&todo_schema::STICKY, on) {
            self.as_mut().set_toast(QString::from(e));
        }
    }

    pub fn color_presets(&self) -> QString {
        QString::from(organiser_core::color::presets_json())
    }

    pub fn edit_due(self: Pin<&mut Self>, id: i64, due: &QString) {
        let v = due.to_string();
        self.act(|c| c.edit_due(id, &v));
    }

    pub fn edit_priority(self: Pin<&mut Self>, id: i64, priority: &QString) {
        let v = priority.to_string();
        self.act(|c| c.edit_priority(id, &v));
    }

    pub fn edit_epic(self: Pin<&mut Self>, id: i64, epic: i64) {
        self.act(|c| c.edit_epic(id, opt(epic)));
    }

    /// `tags` is a comma-separated list.
    pub fn set_task_tags(self: Pin<&mut Self>, id: i64, tags: &QString) {
        let list: Vec<String> = tags
            .to_string()
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();
        self.act(|c| c.set_tags(id, &list));
    }

    pub fn create_tag(self: Pin<&mut Self>, name: &QString, parent: &QString) {
        let (n, p) = (name.to_string(), parent.to_string());
        self.act(|c| c.create_tag(&n, &p));
    }

    pub fn set_tag_color(self: Pin<&mut Self>, name: &QString, color: &QString) {
        let (n, col) = (name.to_string(), color.to_string());
        self.act(|c| c.set_tag_color(&n, &col));
    }

    pub fn set_tag_parent(self: Pin<&mut Self>, name: &QString, parent: &QString) {
        let (n, p) = (name.to_string(), parent.to_string());
        self.act(|c| c.set_tag_parent(&n, &p));
    }

    pub fn delete_tag(self: Pin<&mut Self>, name: &QString, with_children: bool) {
        let n = name.to_string();
        self.act(|c| c.delete_tag(&n, with_children));
    }

    pub fn create_epic(mut self: Pin<&mut Self>, title: &QString) -> i64 {
        let title = title.to_string();
        let mut id = -1;
        self.as_mut().act(|c| {
            id = c.create_epic(&title)?;
            Ok(())
        });
        id
    }

    pub fn epic_info(&self, id: i64) -> QString {
        self.core
            .as_ref()
            .and_then(|c| c.epic_json(id).ok())
            .map_or_else(|| QString::from("{}"), QString::from)
    }

    pub fn set_epic_title(self: Pin<&mut Self>, id: i64, title: &QString) {
        let v = title.to_string();
        self.act(|c| c.set_epic_title(id, &v));
    }

    pub fn set_epic_notes(self: Pin<&mut Self>, id: i64, notes: &QString) {
        let v = notes.to_string();
        self.act(|c| c.set_epic_notes(id, &v));
    }

    pub fn set_epic_due(self: Pin<&mut Self>, id: i64, due: &QString) {
        let v = due.to_string();
        self.act(|c| c.set_epic_due(id, &v));
    }

    pub fn set_epic_color(self: Pin<&mut Self>, id: i64, color: &QString) {
        let v = color.to_string();
        self.act(|c| c.set_epic_color(id, &v));
    }

    pub fn set_epic_icon(self: Pin<&mut Self>, id: i64, icon: &QString) {
        let v = icon.to_string();
        self.act(|c| c.set_epic_icon(id, &v));
    }

    pub fn delete_epic(self: Pin<&mut Self>, id: i64, with_tasks: bool) {
        self.act(|c| c.delete_epic(id, with_tasks));
    }

    pub fn create_status(self: Pin<&mut Self>, name: &QString, color: &QString, closed: bool) {
        let (n, col) = (name.to_string(), color.to_string());
        self.act(|c| c.create_status(&n, &col, closed));
    }

    pub fn edit_status(
        self: Pin<&mut Self>,
        id: i64,
        name: &QString,
        color: &QString,
        closed: bool,
    ) {
        let (n, col) = (name.to_string(), color.to_string());
        self.act(|c| c.edit_status(id, &n, &col, closed));
    }

    pub fn move_status(self: Pin<&mut Self>, id: i64, step: i32) {
        self.act(|c| c.move_status(id, step));
    }

    pub fn delete_status(self: Pin<&mut Self>, id: i64) {
        self.act(|c| c.delete_status(id));
    }
}
