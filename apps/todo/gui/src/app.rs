//! State, messages and update. Every change goes through `todo-schema` and the engine; the view
//! never touches the database.

use std::collections::{BTreeMap, HashMap};

use chrono::NaiveDate;
use iced::widget::text_editor;
use iced::{Subscription, Task};
use organiser_core::paths;
use organiser_core::settings::StartView;
use organiser_engine::{Db, Filters, Node, Tag, build_tree};
use organiser_ui::components::tag_query::{count as tag_count, engine_filter, retain_existing};
use organiser_ui::iad::primitives::color_picker::Hsva;
use organiser_ui::iad::primitives::rich_text as rt;
use organiser_ui::iad::primitives::tag_groups::{GroupEvent, TagGroups};
use organiser_ui::iad::{self, Scheme};
use organiser_ui::primitives::expand::{Folds, flat_rows};
use organiser_ui::primitives::scheme::Mode;
use organiser_ui::shell;
use organiser_ui::primitives::search::Search;
use organiser_ui::primitives::text::ellipsize;
use organiser_ui::widgets::preferences::{Changed, PrefMsg, Preferences};
use organiser_ui::widgets::sticky_link::StickyLink;
use organiser_ui::widgets::tag_manager::{TagManager, TagMsg};
use todo_schema::{EPIC, SECTION};
use todo_schema::board::BoardView;
use todo_schema::lifecycle::{self, NewTask};
use todo_schema::priority::Priority;
use todo_schema::quick;
use todo_schema::sort::{SortKey, sort_tasks};
use todo_schema::status::{Status, Statuses};
use todo_schema::statuses::{self, StatusEdit};
use todo_schema::task::{Edit, Epic, EpicSummary, Section, SectionSummary, Task as TodoTask};
use todo_schema::vault::Vault;
use todo_schema::views::{self, Filter, Scope};

#[derive(Debug, Clone)]
pub enum Message {
    SystemTheme(iad::utils::ThemeSettings),
    Window(iad::windows::Action),
    Quit,
    // What the list shows.
    /// The header toggle: only tasks that are current, or all of them.
    Current(bool),
    ShowClosed(bool),
    /// Show every task at one level, subtasks included.
    Flat(bool),
    /// Show the tasks as notes, a board or a list.
    SetView(View),
    /// Order the tasks by this.
    SetSort(SortKey),
    /// List subtasks as entries of their own (true), or only top-level tasks.
    ShowSubtasks(bool),
    /// In the views of every task, group them by epic after the pinned ones.
    GroupByEpic(bool),
    /// On an epic's page, group its tasks by section.
    GroupBySection(bool),
    /// Open or close the board's sidebar when the window is too narrow to show it.
    Drawer(bool),
    /// Hide or show the docked sidebar of a wide window.
    ToggleSidebar,
    /// A board card was picked up.
    DragStart(i64),
    /// The card being dragged was dropped on this status's column.
    DropOn(i64),
    /// The pointer button went up: whatever was being dragged is let go.
    DragEnd,
    /// The task being dragged was dropped on a section of its epic, or (`None`) on no section.
    DropInSection(Option<i64>),
    /// An epic of the sidebar is picked up, to move it.
    EpicDragStart(i64),
    /// The picked-up epic is dropped before or after this one.
    DropEpic(i64, organiser_ui::iad::primitives::drag::Place),
    /// A task dropped on an epic's group (or on "Unassigned", `None`): it moves to that epic.
    DropInEpic(Option<i64>),
    /// A section was picked up by its name, to put it in another place.
    SectionDragStart(i64),
    /// The task being dragged was dropped on another task: it becomes that task's subtask.
    DropOnTask(i64),
    /// The task being dragged was dropped on open ground: it leaves its parent and stands alone.
    DropOut,
    /// Take a subtask out of its parent, to stand as a task of its own.
    DConvert(i64),
    /// The answer to "delete this epic?": keep its tasks (they leave the epic), delete them too,
    /// or neither.
    EpicDeleteKeep,
    EpicDeleteAll,
    EpicDeleteCancel,
    OpenFilter,
    CloseFilter,
    ClearFilters,
    /// Show one epic's tasks, or (`None`) all of them: the sidebar's selection.
    SetEpic(Option<i64>),
    /// The tag filter's groups changed.
    TagGroups(GroupEvent<String>),
    OpenPrefs,
    ClosePrefs,
    Pref(PrefMsg),
    /// Todo's own settings: what it opens on, and whether it starts with finished tasks.
    PrefStartView(StartView),
    PrefShowClosed(bool),
    PrefShowSubtasks(bool),
    PrefGroupByEpic(bool),
    PrefGroupBySection(bool),
    PrefDetailsDrawer(bool),
    /// The archived tasks, grouped by epic.
    Archived,
    /// The tasks that are in no epic.
    Unassigned,
    /// Archive the selected task (true) or bring it back.
    DArchive(bool),
    /// Read the files of the vault, then write the tasks out.
    SyncVault,
    /// The window was focused: files may have been changed meanwhile.
    FocusGained,
    /// Pick the directory the tasks live in.
    ChooseVault,
    VaultChosen(Option<std::path::PathBuf>),
    /// The folder `Todo` in the documents folder.
    UseDefaultVault,
    /// Keep the tasks in the database alone again; the files stay where they are.
    VaultOff,
    /// Show the vault in the file manager.
    OpenVault,
    /// Pick Markdown files to bring into the vault.
    AddFiles,
    FilesPicked(Vec<std::path::PathBuf>),
    /// Show the selected task's file in the file manager.
    ShowFile,
    /// Link (true) or unlink this app to the Sticky Notes app.
    PrefStickyLink(bool),
    /// Show the selected task as a desktop sticky (true) or stop.
    DSticky(bool),
    /// Pin the selected task to the top of every view (true) or unpin it.
    DPin(bool),
    /// The filters the schema offers changed.
    Fields(Filters),
    Tags(TagMsg),
    TagQuery(String),
    EditEpic(i64),
    NewEpic,
    ToggleSearch,
    SearchInput(String),
    Escape,
    /// Open the edit page on a new task. Nothing is saved until `DCreate`.
    New,
    /// Save the new task being edited.
    DCreate,
    Toggle(i64),
    Fold(i64),
    // The details of the selected task or epic.
    Select(i64),
    CloseDetails,
    DTitle(String),
    DNotes(text_editor::Action),
    /// Show the details drawn (true) or their Markdown source (false).
    DPreview(bool),
    /// Task `n` of the drawn details was ticked or cleared.
    DTask(usize, bool),
    /// A link in the drawn details was pressed.
    OpenLink(String),
    DStart(String),
    DDue(String),
    DStatus(Status),
    DPriority(Priority),
    DEpic(EpicChoice),
    /// The section of the task's epic it is in, or none.
    DSection(Option<i64>),
    /// The colour of the epic or section open in the details, picked or typed.
    DColor(Hsva),
    DColorText(String),
    /// The icon of the epic open in the details, by name; none is the folder.
    DIcon(Option<&'static str>),
    /// Add a section to an epic, and open it to be named.
    NewSection(i64),
    /// Fold a section of the epic page, or open it again.
    SectionFold(i64),
    /// A new task in this section of the epic being viewed.
    NewInSection(i64),
    DTagStart,
    DTagText(String),
    /// Add a tag; an empty name stops adding.
    DTagAdd(String),
    DTagRemove(String),
    DSubText(String),
    DSubAdd,
    DDelete,
    // The statuses dialog.
    StatusNew,
    StatusName(i64, String),
    StatusColorText(i64, String),
    StatusColor(i64, Hsva),
    StatusClosed(i64, bool),
    StatusMove(i64, i32),
    StatusDelete(i64),
    DismissToast,
    /// A development screenshot was saved (or failed); the app then quits.
    Captured,
}

/// How the tasks are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Every task a note, in a grid of cards.
    Notes,
    /// A kanban board, a column per status.
    Board,
    /// The nested list.
    List,
    /// The Eisenhower matrix: four boxes, each a compact list.
    Quadrants,
}

impl View {
    pub const ALL: [View; 4] = [View::Notes, View::Board, View::List, View::Quadrants];

    pub fn label(self) -> &'static str {
        match self {
            View::Notes => "Notes",
            View::Board => "Kanban board",
            View::List => "List",
            View::Quadrants => "Quadrants",
        }
    }
}

impl From<StartView> for View {
    fn from(v: StartView) -> Self {
        match v {
            StartView::Notes => View::Notes,
            StartView::Board => View::Board,
            StartView::List => View::List,
            StartView::Quadrants => View::Quadrants,
        }
    }
}

/// One entry of the epic dropdown: an epic, or none.
#[derive(Debug, Clone, PartialEq)]
pub struct EpicChoice {
    pub id: Option<i64>,
    pub title: String,
}

impl std::fmt::Display for EpicChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&ellipsize(&self.title, 36))
    }
}

pub enum Subject {
    Task(TodoTask),
    Epic(Epic),
    Section(Section),
}

/// The selected task or epic. The text fields hold what the person typed; each edit is saved at
/// once, so there is no Save button and nothing to lose.
pub struct Details {
    pub subject: Subject,
    pub title: String,
    pub notes: text_editor::Content,
    /// Draw the notes instead of editing their source.
    pub preview: bool,
    /// The notes drawn, kept so a redraw does not parse them again.
    pub doc: Vec<rt::Block>,
    pub start: String,
    pub due: String,
    /// The inline tag entry is showing.
    pub adding_tag: bool,
    pub tag_text: String,
    pub sub_text: String,
    pub subtasks: Vec<TodoTask>,
    pub error: Option<String>,
}

impl Details {
    /// Whether this is a task being written, not yet saved: the edit page doubles as the page
    /// that creates one, and a draft has no id.
    pub fn is_new(&self) -> bool {
        matches!(&self.subject, Subject::Task(t) if t.id == 0)
    }

    pub fn id(&self) -> i64 {
        match &self.subject {
            Subject::Task(t) => t.id,
            Subject::Epic(e) => e.id,
            Subject::Section(s) => s.id,
        }
    }

    pub fn tags(&self) -> &[String] {
        match &self.subject {
            Subject::Task(t) => &t.tags,
            Subject::Epic(e) => &e.tags,
            Subject::Section(_) => &[],
        }
    }
}

pub struct App {
    db: Db,
    pub today: NaiveDate,
    pub scope: Scope,
    pub epic_filter: Option<i64>,
    /// Only the tasks in no epic.
    pub no_epic: bool,
    /// How many tasks are in no epic still open, and in all.
    pub unassigned: (usize, usize),
    /// The directory the tasks live in as files, when they do.
    pub vault: Option<Vault>,
    /// The file each task had after the last flush: how a task deleted for good is known to have
    /// had one.
    pub vault_known: BTreeMap<i64, String>,
    pub db_path: std::path::PathBuf,
    /// Tasks must have at least one of these tags (or a tag below one of them).
    /// The picked tags: groups that are alternatives, every tag of one needed.
    pub tag_groups: TagGroups<String>,
    /// The shared settings and the Preferences sheet over them.
    pub prefs: Preferences,
    /// Whether Sticky Notes is installed and this app is linked to it.
    pub sticky: StickyLink,
    /// What was chosen in the filters the schema offers (status, priority, …).
    pub filters: Filters,
    pub tag_manager: TagManager,
    pub filter_open: bool,
    /// What was typed to narrow the tag chips in the filter.
    pub tag_query: String,
    pub show_closed: bool,
    /// Tasks and their subtasks are equal rows, with no nesting.
    pub flat: bool,
    /// The kanban board instead of the list.
    pub view: View,
    /// Subtasks listed as entries of their own.
    pub show_subtasks: bool,
    /// Group the tasks by epic in the views of every task.
    pub group_by_epic: bool,
    /// Group an epic's tasks by section, on its page.
    pub group_by_section: bool,
    /// What the tasks are ordered by (pinned ones are always first).
    pub sort: SortKey,
    /// The notes of the tasks the notes view shows, drawn, by task id.
    pub docs: HashMap<i64, Vec<rt::Block>>,
    /// The board's tasks: every status, whatever the list's own filters hide.
    pub board: Vec<TodoTask>,
    /// `(done, total)` subtasks of each board task that has any.
    pub sub_progress: HashMap<i64, (usize, usize)>,
    pub drawer_open: bool,
    /// The sidebar of a wide window is hidden.
    pub sidebar_hidden: bool,
    /// Development only: draw the header menu open (for screenshots).
    pub menu_open: bool,
    /// The listed tasks as equal rows, for flat mode and for the current tasks.
    pub flat_tasks: Vec<TodoTask>,
    /// Titles of the listed tasks and their parents, to say where a flat row's task belongs.
    pub task_titles: HashMap<i64, String>,
    /// The person's statuses, in board order.
    pub statuses: Statuses,
    /// What each status's colour entry shows, so typing is not interrupted.
    pub status_text: HashMap<i64, String>,
    /// What each status's name entry shows: a name can be cleared and retyped without each
    /// keystroke having to be a valid name.
    pub status_names: HashMap<i64, String>,
    pub search: Search,
    pub forest: Vec<Node<TodoTask>>,
    pub epics: Vec<EpicSummary>,
    pub epic_titles: HashMap<i64, String>,
    pub epic_choices: Vec<EpicChoice>,
    /// The sections of each epic, by epic id.
    /// The task being dragged across the board.
    pub dragging: Option<i64>,
    /// The section being dragged to another place.
    pub dragging_section: Option<i64>,
    /// The epic being carried to another place in the sidebar.
    pub dragging_epic: Option<i64>,
    /// An epic with tasks that is about to be deleted, waiting for the answer to what becomes of
    /// its tasks.
    pub confirm_epic_delete: Option<i64>,
    /// What the dragged task cannot be dropped on: itself and everything below it.
    pub drag_blocked: std::collections::HashSet<i64>,
    pub epic_sections: HashMap<i64, Vec<SectionSummary>>,
    /// Sections folded shut on the epic page.
    pub folded_sections: std::collections::HashSet<i64>,
    pub tags: Vec<Tag>,
    pub tag_names: Vec<String>,
    pub folds: Folds,
    pub details: Option<Details>,
    pub toast: Option<String>,
    pub desktop_dark: bool,
    pub mode: Mode,
}

impl App {
    pub fn boot() -> (Self, Task<Message>) {
        let args: Vec<String> = std::env::args().collect();
        let path = args
            .iter()
            .position(|a| a == "--db")
            .and_then(|i| args.get(i + 1))
            .map_or_else(
                || paths::db_path(&todo_schema::META),
                std::path::PathBuf::from,
            );
        let mut db = Db::open(&path, &todo_schema::SCHEMA).unwrap_or_else(|e| {
            eprintln!("error: cannot open {}: {e}", path.display());
            std::process::exit(1);
        });
        // A new database gets its statuses; an older one has its tasks moved onto them.
        if let Err(e) = statuses::ensure_defaults(&mut db) {
            eprintln!("error: cannot set up statuses: {e}");
            std::process::exit(1);
        }
        let prefs = Preferences::load();
        let mode = Mode::from(prefs.settings.appearance.theme);
        let view_by_default = View::from(prefs.settings.todo.start_view);
        let show_subtasks_by_default = prefs.settings.todo.show_subtasks;
        let prefs_group_by_epic = prefs.settings.todo.group_by_epic;
        let prefs_group_by_section = prefs.settings.todo.group_by_section;
        let show_closed_by_default = prefs.settings.todo.show_closed;
        let mut app = Self {
            db,
            today: views::today(),
            scope: Scope::All,
            epic_filter: None,
            no_epic: false,
            unassigned: (0, 0),
            vault: None,
            vault_known: BTreeMap::new(),
            db_path: path.clone(),
            tag_groups: TagGroups::new(),
            prefs,
            sticky: StickyLink::load(&todo_schema::STICKY),
            filters: Filters::default(),
            tag_manager: TagManager::default(),
            filter_open: false,
            tag_query: String::new(),
            show_closed: show_closed_by_default,
            flat: false,
            view: view_by_default,
            show_subtasks: show_subtasks_by_default,
            group_by_epic: prefs_group_by_epic,
            group_by_section: prefs_group_by_section,
            sort: SortKey::default(),
            docs: HashMap::new(),
            menu_open: false,
            board: Vec::new(),
            sub_progress: HashMap::new(),
            drawer_open: false,
            sidebar_hidden: false,
            flat_tasks: Vec::new(),
            task_titles: HashMap::new(),
            statuses: Statuses::default(),
            status_text: HashMap::new(),
            status_names: HashMap::new(),
            search: Search::default(),
            forest: Vec::new(),
            epics: Vec::new(),
            epic_titles: HashMap::new(),
            epic_choices: Vec::new(),
            dragging: None,
            dragging_section: None,
            dragging_epic: None,
            confirm_epic_delete: None,
            drag_blocked: Default::default(),
            epic_sections: HashMap::new(),
            folded_sections: Default::default(),
            tags: Vec::new(),
            tag_names: Vec::new(),
            folds: Folds::default(),
            details: None,
            toast: None,
            desktop_dark: false,
            mode,
        };
        // Development switches, for screenshots of states that need a click to reach.
        if let Ok(q) = std::env::var("TODO_GUI_SEARCH") {
            app.search.toggle();
            app.search.input(q);
        }
        app.show_closed |= std::env::var_os("TODO_GUI_CLOSED").is_some();
        app.flat = std::env::var_os("TODO_GUI_FLAT").is_some();
        app.menu_open = std::env::var_os("TODO_GUI_MENU_OPEN").is_some();
        if std::env::var_os("TODO_GUI_LIST").is_some() {
            app.view = View::List;
        }
        app.confirm_epic_delete = std::env::var("TODO_GUI_DELETE_EPIC")
            .ok()
            .and_then(|v| v.parse().ok());
        if std::env::var_os("TODO_GUI_UNGROUPED").is_some() {
            app.group_by_epic = false;
            app.group_by_section = false;
        }
        match std::env::var("TODO_GUI_SORT").as_deref() {
            Ok("priority") => app.sort = SortKey::Priority,
            Ok("created") => app.sort = SortKey::Created,
            Ok("due") => app.sort = SortKey::Due,
            _ => {}
        }
        match std::env::var("TODO_GUI_VIEW").as_deref() {
            Ok("notes") => app.view = View::Notes,
            Ok("board") => app.view = View::Board,
            Ok("list") => app.view = View::List,
            Ok("quadrants") => app.view = View::Quadrants,
            _ => {}
        }
        if std::env::var_os("TODO_GUI_CURRENT").is_some() {
            app.scope = Scope::Current;
        }
        if std::env::var_os("TODO_GUI_ARCHIVED").is_some() {
            app.scope = Scope::Archived;
        }
        app.epic_filter = std::env::var("TODO_GUI_EPIC")
            .ok()
            .and_then(|v| v.parse().ok());
        if let Ok(t) = std::env::var("TODO_GUI_TAG") {
            app.tag_groups =
                TagGroups::from_parts(vec![t.split(',').map(String::from).collect()], 0);
        }
        app.filter_open = std::env::var_os("TODO_GUI_FILTER").is_some();
        app.reload();
        if let Some(id) = std::env::var("TODO_GUI_SELECT")
            .ok()
            .and_then(|v| v.parse().ok())
        {
            app.open(id);
        }
        if std::env::var_os("TODO_GUI_ADD_TAG").is_some() {
            app.with_details(|d| d.adding_tag = true);
        }
        if std::env::var_os("TODO_GUI_NEW").is_some() || shell::wants_new(&todo_schema::META) {
            let _ = app.update(Message::New);
        }
        if let Ok(page) = std::env::var("TODO_GUI_PREFS") {
            app.prefs.open = true;
            // Asking for a page opens it, on a narrow window too.
            app.prefs.page_open = true;
            app.prefs.page = match page.as_str() {
                "tags" => organiser_ui::widgets::preferences::Page::Tags,
                "statuses" => organiser_ui::widgets::preferences::Page::Extra,
                "app" => organiser_ui::widgets::preferences::Page::App,
                _ => organiser_ui::widgets::preferences::Page::Appearance,
            };
        }
        match std::env::var("TODO_GUI_THEME").as_deref() {
            Ok("light") => app.mode = Mode::Light,
            Ok("dark") => app.mode = Mode::Dark,
            _ => {}
        }
        // Select a task a moment after start, so the details pane slides in on screen.
        let later: Option<i64> = std::env::var("TODO_GUI_SELECT_LATER")
            .ok()
            .and_then(|v| v.parse().ok());
        let dev = iad::dev::Switches::from_env();
        dev.apply();
        let mut tasks = vec![iad::utils::detect_theme().map(Message::SystemTheme)];
        if dev.screenshot.is_some() {
            tasks.push(
                iced::window::oldest()
                    .and_then(move |id| dev.capture(id).unwrap_or_else(Task::none))
                    .map(|_| Message::Captured),
            );
        }
        if let Some(id) = later {
            tasks.push(
                Task::future(async {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                })
                .map(move |()| Message::Select(id)),
            );
        }
        // `TODO_GUI_VAULT` picks one for a trial; set to nothing it means none, whatever the
        // settings say.
        let dir = match std::env::var_os("TODO_GUI_VAULT") {
            Some(v) if v.is_empty() => None,
            Some(v) => Some(std::path::PathBuf::from(v)),
            None => app.prefs.settings.todo.vault_dir.clone(),
        };
        if let Some(dir) = dir {
            app.open_vault(dir, &path);
        }
        (app, Task::batch(tasks))
    }

    pub fn title(&self) -> String {
        "Todo".into()
    }

    pub fn theme(&self) -> iad::Theme {
        organiser_ui::shell::theme(self.mode.is_dark(self.desktop_dark))
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iad::utils::follow_theme().map(Message::SystemTheme),
            // Letting go anywhere ends a drag. The column it landed on has already been told.
            iced::event::listen_with(|event, _, _| match event {
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )) => Some(Message::DragEnd),
                // Coming back to the window: a file may have been changed in another editor.
                iced::Event::Window(iced::window::Event::Focused) => Some(Message::FocusGained),
                _ => None,
            }),
        ])
    }

    fn filter(&self) -> Filter {
        Filter {
            scope: self.scope,
            show_closed: self.show_closed,
            text: self.search.query().map(String::from),
            // The picked tags are alternatives: a task matches if it has any one of them.
            tags: engine_filter(&self.tag_groups),
            epic: self.epic_filter,
            no_epic: self.no_epic,
            fields: self.filters.clone(),
            include_archived: false,
            hide_subtasks: !self.show_subtasks,
            sort: self.sort,
        }
    }

    /// Keep the tasks in `dir` as files: read what is there, write the rest, and say what changed.
    fn open_vault(&mut self, dir: std::path::PathBuf, db_path: &std::path::Path) {
        match Vault::open(dir, Some(db_path)) {
            Ok(v) => {
                self.vault = Some(v);
                self.vault_sync(false);
            }
            Err(e) => {
                self.vault = None;
                self.toast = Some(e.to_string());
            }
        }
    }

    /// Read the vault's files into the tasks, then write the tasks' changes to the files.
    fn vault_sync(&mut self, say: bool) {
        let Some(v) = self.vault.clone() else { return };
        let result = v.sync(&mut self.db).and_then(|mut r| {
            r.add(v.flush(&mut self.db, &self.vault_known)?);
            Ok(r)
        });
        match result {
            Ok(r) => {
                self.vault_known = v.known(&self.db).unwrap_or_default();
                if say || r.created + r.updated + r.moved + r.trashed + r.conflicts > 0 {
                    self.toast = Some(r.summary_with("archived"));
                }
                if let Some(w) = r.warnings.first() {
                    self.toast = Some(w.clone());
                }
                self.refresh_details();
                self.reload();
            }
            Err(e) => self.fail(e),
        }
    }

    /// After a change: write it to the files.
    fn vault_flush(&mut self) {
        let Some(v) = self.vault.clone() else { return };
        match v.flush(&mut self.db, &self.vault_known) {
            Ok(r) => {
                self.vault_known = v.known(&self.db).unwrap_or_default();
                if let Some(w) = r.warnings.first() {
                    self.toast = Some(w.clone());
                }
            }
            Err(e) => self.toast = Some(e.to_string()),
        }
    }

    fn set_vault(&mut self, dir: Option<std::path::PathBuf>) {
        self.prefs.settings.todo.vault_dir = dir.clone();
        if let Err(e) = self.prefs.save() {
            self.fail(e);
        }
        match dir {
            Some(d) => {
                let db_path = self.db_path.clone();
                self.open_vault(d, &db_path);
                if self.vault.is_none() {
                    self.prefs.settings.todo.vault_dir = None;
                    let _ = self.prefs.save();
                }
            }
            None => {
                self.vault = None;
                self.vault_known.clear();
            }
        }
    }

    /// Re-read what the lists show from the database.
    fn reload(&mut self) {
        self.vault_flush();
        self.today = views::today();
        self.unassigned = views::unassigned(&self.db).unwrap_or((0, 0));
        match statuses::load(&self.db) {
            Ok(s) => self.statuses = s,
            Err(e) => self.toast = Some(e.to_string()),
        }
        match views::tasks(&self.db, &self.filter(), self.today) {
            Ok(tasks) => {
                self.forest = build_tree(tasks.clone(), |t| t.id, |t| t.parent_id);
                // The flat list: "current" is already in deadline order and each task stands
                // alone; otherwise it is the tree read top to bottom.
                self.flat_tasks = if self.scope == Scope::Current {
                    tasks
                } else {
                    flat_rows(&self.forest)
                        .iter()
                        .map(|r| r.value.clone())
                        .collect()
                };
            }
            Err(e) => self.toast = Some(e.to_string()),
        }
        // The notes and the board both read every task, closed ones included.
        if self.view != View::List {
            self.reload_board();
        }
        self.task_titles = self.titles_of(&self.forest);
        if let Ok(all) = views::epics(&self.db) {
            self.epic_titles = all
                .iter()
                .map(|s| (s.epic.id, s.epic.title.clone()))
                .collect();
            self.epic_choices = std::iter::once(EpicChoice {
                id: None,
                title: "No epic".into(),
            })
            .chain(
                all.iter()
                    .map(|s| EpicChoice {
                        id: Some(s.epic.id),
                        title: s.epic.title.clone(),
                    }),
            )
            .collect();
            self.epic_sections = all
                .iter()
                .filter_map(|s| {
                    views::sections(&self.db, s.epic.id)
                        .ok()
                        .map(|v| (s.epic.id, v))
                })
                .collect();
            self.epics = all;
        }
        // Every tag has a colour of its own, drawn the same everywhere.
        let _ = self.db.ensure_tag_colors(&self.prefs.settings.palette());
        if let Ok(t) = self.db.tags() {
            organiser_ui::components::chip::set_tag_colors(&t);
            self.tag_manager.sync(&t, &self.prefs.settings.palette());
            // A tag that is gone no longer filters.
            self.tag_groups = retain_existing(&self.tag_groups, |f| t.iter().any(|x| x.name == f));
            self.tag_names = t.iter().map(|t| t.name.clone()).collect();
            self.tags = t;
        }
    }

    /// The tasks of the board and of the notes, and how far along each one's subtasks are. They
    /// show every status (each view chooses what to show), so they ask for closed tasks whatever
    /// the list is hiding, and for subtasks whatever "show subtasks" says, to count them.
    fn reload_board(&mut self) {
        // Current tasks narrow it like any other filter.
        let f = Filter {
            show_closed: true,
            hide_subtasks: false,
            ..self.filter()
        };
        match views::tasks(&self.db, &f, self.today) {
            Ok(tasks) => {
                let mut progress: HashMap<i64, (usize, usize)> = HashMap::new();
                for t in &tasks {
                    if let Some(p) = t.parent_id {
                        let e = progress.entry(p).or_default();
                        e.1 += 1;
                        e.0 += usize::from(self.statuses.is_closed(t.status));
                    }
                }
                self.sub_progress = progress;
                let tasks: Vec<TodoTask> = tasks
                    .into_iter()
                    .filter(|t| self.show_subtasks || t.parent_id.is_none())
                    .collect();
                // The notes as they read, drawn once, for the cards that show them.
                self.docs = if self.view == View::Notes {
                    tasks
                        .iter()
                        .filter(|t| !t.notes.is_empty())
                        .map(|t| (t.id, organiser_ui::components::rich::render(&t.notes)))
                        .collect()
                } else {
                    HashMap::new()
                };
                // Pinned first, then by the chosen order: all the cards as one list, not a tree.
                let mut tasks = tasks;
                let subtasks: HashMap<i64, usize> =
                    self.sub_progress.iter().map(|(id, (_, n))| (*id, *n)).collect();
                sort_tasks(&mut tasks, self.sort, &subtasks);
                self.board = tasks;
            }
            Err(e) => self.toast = Some(e.to_string()),
        }
    }

    /// The titles of every listed task, plus those of their parents (which a filter may have
    /// left out of the list).
    fn titles_of(&self, forest: &[Node<TodoTask>]) -> HashMap<i64, String> {
        fn walk(nodes: &[Node<TodoTask>], out: &mut HashMap<i64, String>, parents: &mut Vec<i64>) {
            for n in nodes {
                out.insert(n.value.id, n.value.title.clone());
                parents.extend(n.value.parent_id);
                walk(&n.children, out, parents);
            }
        }
        let (mut out, mut parents) = (HashMap::new(), Vec::new());
        walk(forest, &mut out, &mut parents);
        for id in parents {
            if let std::collections::hash_map::Entry::Vacant(e) = out.entry(id)
                && let Ok(item) = self.db.get(id)
            {
                e.insert(item.title);
            }
        }
        out
    }

    fn fail(&mut self, e: impl std::fmt::Display) {
        self.toast = Some(e.to_string());
    }

    /// The columns the board shows: the open statuses, or every status while closed tasks are on.
    pub fn board_view(&self) -> BoardView {
        if self.show_closed {
            BoardView::All
        } else {
            BoardView::Open
        }
    }

    /// Whether the list is flat: when the person asked for it, and always for the current tasks,
    /// which are each shown on their own rather than with their whole family.
    pub fn is_flat(&self) -> bool {
        self.flat || self.scope == Scope::Current
    }

    /// Whether the views of every task are in groups, one per epic: the option is on, the page is
    /// All (not Current, not one epic's) and there are epics.
    pub fn grouped_by_epic(&self) -> bool {
        self.group_by_epic
            && matches!(self.scope, Scope::All | Scope::Archived)
            && self.epic_filter.is_none()
            && !self.no_epic
            && !self.epics.is_empty()
    }

    /// The name of an epic's icon, if it has chosen one.
    pub fn epic_icon(&self, id: i64) -> Option<&str> {
        self.epics
            .iter()
            .find(|s| s.epic.id == id)
            .and_then(|s| s.epic.icon.as_deref())
    }

    /// The colour of an epic, if it has been given one.
    pub fn epic_color(&self, id: i64) -> Option<iced::Color> {
        self.epics
            .iter()
            .find(|s| s.epic.id == id)
            .and_then(|s| s.epic.color.as_deref())
            .and_then(organiser_ui::components::dot::color_from_hex)
    }

    /// The epic being viewed is a page, not a filter, so it stays.
    fn clear_filters(&mut self) {
        self.tag_groups = TagGroups::new();
        self.filters.clear();
    }

    /// How many filters (tags, the schema's own) narrow the list. The epic is the view, not one.
    pub fn filter_count(&self) -> usize {
        tag_count(&self.tag_groups) + self.filters.count()
    }

    // ---- details ---------------------------------------------------------------------------

    /// Show a task or an epic in the details pane.
    fn open(&mut self, id: i64) {
        let item = match self.db.get(id) {
            Ok(i) => i,
            Err(e) => return self.fail(e),
        };
        let (subject, notes, start, due) = if item.kind == EPIC {
            let e = Epic::from(&item);
            let (n, s, d) = (e.notes.clone(), e.start, e.due);
            (Subject::Epic(e), n, s, d)
        } else if item.kind == SECTION {
            let s = Section::from(&item);
            (Subject::Section(s.clone()), s.notes, None, None)
        } else {
            let t = TodoTask::from(&item);
            let (n, s, d) = (t.notes.clone(), t.start, t.due);
            (Subject::Task(t), n, s, d)
        };
        self.details = Some(Details {
            subject,
            title: item.title,
            preview: std::env::var_os("TODO_GUI_PREVIEW").is_some(),
            doc: organiser_ui::components::rich::render(&notes),
            notes: text_editor::Content::with_text(&notes),
            start: start.map(|d| d.to_string()).unwrap_or_default(),
            due: due.map(|d| d.to_string()).unwrap_or_default(),
            adding_tag: false,
            tag_text: String::new(),
            sub_text: String::new(),
            subtasks: Vec::new(),
            error: None,
        });
        self.refresh_details();
    }

    /// Re-read the selected item (its status, tags, subtasks) without touching what is typed.
    fn refresh_details(&mut self) {
        let Some(d) = &self.details else { return };
        if d.is_new() {
            return;
        }
        let id = d.id();
        match self.db.get(id) {
            Ok(item) => {
                let subtasks = views::subtasks(&self.db, id).unwrap_or_default();
                if let Some(d) = &mut self.details {
                    d.subject = if item.kind == EPIC {
                        Subject::Epic(Epic::from(&item))
                    } else if item.kind == SECTION {
                        Subject::Section(Section::from(&item))
                    } else {
                        Subject::Task(TodoTask::from(&item))
                    };
                    d.subtasks = subtasks;
                }
            }
            Err(_) => self.details = None,
        }
    }

    /// A date field: empty clears it, anything else must read as a date.
    fn date(&self, text: &str) -> Result<Option<NaiveDate>, String> {
        if text.trim().is_empty() {
            Ok(None)
        } else {
            quick::date(text, self.today)
                .map(Some)
                .ok_or_else(|| "Use a date like 2026-12-01".to_string())
        }
    }

    /// Save the typed fields. Called after every edit.
    fn commit(&mut self) {
        let Some(d) = &self.details else { return };
        let (start, due) = match (self.date(&d.start), self.date(&d.due)) {
            (Ok(s), Ok(e)) => (s, e),
            (Err(m), _) | (_, Err(m)) => {
                if let Some(d) = &mut self.details {
                    d.error = Some(m);
                }
                return;
            }
        };
        if d.is_new() {
            // A draft is kept as typed until it is created; only the dates are checked.
            if let Some(d) = &mut self.details {
                d.error = None;
                d.doc = organiser_ui::components::rich::render(&d.notes.text());
            }
            return;
        }
        let edit = Edit {
            title: (!d.title.trim().is_empty()).then(|| d.title.clone()),
            notes: Some(d.notes.text().trim_end().to_string()),
            start: Some(start),
            due: Some(due),
            ..Edit::default()
        };
        let id = d.id();
        let result = match d.subject {
            Subject::Task(_) => lifecycle::edit(&mut self.db, id, &edit).map(|_| ()),
            Subject::Epic(_) => lifecycle::edit_epic(&mut self.db, id, &edit).map(|_| ()),
            // A section has no dates.
            Subject::Section(_) => lifecycle::edit_section(
                &mut self.db,
                id,
                &Edit {
                    start: None,
                    due: None,
                    ..edit
                },
            )
            .map(|_| ()),
        };
        if let Some(d) = &mut self.details {
            d.error = result.err().map(|e| e.to_string());
            d.doc = organiser_ui::components::rich::render(&d.notes.text());
        }
        self.refresh_details();
        self.reload();
    }

    fn with_details(&mut self, f: impl FnOnce(&mut Details)) -> bool {
        match &mut self.details {
            Some(d) => {
                d.error = None;
                f(d);
                true
            }
            None => false,
        }
    }

    fn set_tags(&mut self, f: impl FnOnce(&mut Vec<String>)) {
        let Some(d) = &self.details else { return };
        let mut tags = d.tags().to_vec();
        f(&mut tags);
        if d.is_new() {
            if let Some(Details {
                subject: Subject::Task(t),
                ..
            }) = &mut self.details
            {
                t.tags = tags;
            }
            return;
        }
        let id = d.id();
        if let Err(e) = lifecycle::set_tags(&mut self.db, id, &tags)
            && let Some(d) = &mut self.details
        {
            d.error = Some(e.to_string());
        }
        self.refresh_details();
        self.reload();
    }

    fn set_status_color(&mut self, id: i64, hex: String) {
        let e = StatusEdit {
            color: Some(Some(hex)),
            ..StatusEdit::default()
        };
        if let Err(e) = statuses::edit(&mut self.db, id, &e) {
            self.fail(e);
        }
        self.reload();
    }

    /// Colour the epic or section that is open in the details.
    /// `fail`, for the places that must also return a no-op task.
    fn fail_with(&mut self, e: impl std::fmt::Display) -> Task<Message> {
        self.toast = Some(e.to_string());
        Task::none()
    }

    fn set_color(&mut self, hex: String) {
        let Some(d) = &self.details else { return };
        let id = d.id();
        let edit = Edit {
            color: Some(Some(hex)),
            ..Edit::default()
        };
        let result = match d.subject {
            Subject::Epic(_) => lifecycle::edit_epic(&mut self.db, id, &edit).map(|_| ()),
            Subject::Section(_) => lifecycle::edit_section(&mut self.db, id, &edit).map(|_| ()),
            Subject::Task(_) => return,
        };
        if let Err(e) = result {
            self.fail(e);
        }
        self.refresh_details();
        self.reload();
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        // While a task is only a draft the pickers change the draft, and there is nothing yet to
        // add subtasks to or delete.
        if self.details.as_ref().is_some_and(Details::is_new) {
            if let Some(Details {
                subject: Subject::Task(t),
                ..
            }) = &mut self.details
            {
                match &message {
                    Message::DStatus(s) => {
                        t.status = Some(s.id);
                        return Task::none();
                    }
                    Message::DPriority(p) => {
                        t.priority = *p;
                        return Task::none();
                    }
                    Message::DEpic(c) => {
                        if t.epic != c.id {
                            t.section = None;
                        }
                        t.epic = c.id;
                        return Task::none();
                    }
                    Message::DSection(s) => {
                        t.section = *s;
                        return Task::none();
                    }
                    Message::DPin(on) => {
                        t.pinned = *on;
                        return Task::none();
                    }
                    _ => {}
                }
            }
            if matches!(
                message,
                Message::DSubText(_) | Message::DSubAdd | Message::DDelete
            ) {
                return Task::none();
            }
        }
        match message {
            Message::SystemTheme(s) => self.desktop_dark = s.scheme == Scheme::Dark,
            Message::Window(a) => return iced::window::oldest().and_then(move |id| a.perform(id)),
            Message::Quit => return iced::exit(),
            Message::Current(on) => {
                self.scope = if on { Scope::Current } else { Scope::All };
                self.no_epic = false;
                if on {
                    // The sidebar is one choice: current tasks, or an epic, or all.
                    self.epic_filter = None;
                    self.drawer_open = false;
                }
                self.reload();
            }
            Message::Flat(on) => self.flat = on,
            Message::SetView(view) => {
                self.view = view;
                self.drawer_open = false;
                self.reload();
            }
            Message::GroupByEpic(on) => self.group_by_epic = on,
            Message::GroupBySection(on) => self.group_by_section = on,
            Message::PrefGroupBySection(on) => {
                self.prefs.settings.todo.group_by_section = on;
                if let Err(e) = self.prefs.save() {
                    self.fail(e);
                }
            }
            Message::PrefGroupByEpic(on) => {
                self.prefs.settings.todo.group_by_epic = on;
                if let Err(e) = self.prefs.save() {
                    self.fail(e);
                }
            }
            Message::SetSort(sort) => {
                self.sort = sort;
                self.reload();
            }
            Message::ShowSubtasks(on) => {
                self.show_subtasks = on;
                self.reload();
            }
            Message::Drawer(open) => self.drawer_open = open,
            Message::ToggleSidebar => self.sidebar_hidden = !self.sidebar_hidden,
            Message::DragStart(id) => {
                self.dragging = Some(id);
                self.drag_blocked = std::iter::once(id)
                    .chain(
                        lifecycle::descendants(&self.db, id)
                            .unwrap_or_default()
                            .into_iter()
                            .map(|t| t.id),
                    )
                    .collect();
            }
            Message::DragEnd => {
                self.dragging = None;
                self.dragging_section = None;
                self.dragging_epic = None;
            }
            Message::SectionDragStart(id) => self.dragging_section = Some(id),
            Message::EpicDragStart(id) => self.dragging_epic = Some(id),
            Message::DropEpic(target, place) => {
                use organiser_ui::iad::primitives::drag::Place;
                if let Some(moved) = self.dragging_epic.take() {
                    let ids: Vec<i64> = self.epics.iter().map(|s| s.epic.id).collect();
                    // Before the target, or before what follows it (last when nothing does).
                    let before = match place {
                        Place::After => ids
                            .iter()
                            .skip_while(|i| **i != target)
                            .skip(1)
                            .find(|i| **i != moved)
                            .copied(),
                        _ => Some(target),
                    };
                    if moved != target
                        && let Err(e) = lifecycle::move_epic(&mut self.db, moved, before)
                    {
                        self.fail(e);
                    }
                    self.reload();
                }
            }
            Message::DropInEpic(epic) => {
                if let Some(id) = self.dragging.take() {
                    // Dropped on an epic it stands alone there, and its subtasks go with it.
                    if let Err(e) = lifecycle::reparent(&mut self.db, id, None) {
                        return self.fail_with(e);
                    }
                    let edit = Edit {
                        epic: Some(epic),
                        ..Edit::default()
                    };
                    if let Err(e) = lifecycle::edit(&mut self.db, id, &edit) {
                        return self.fail_with(e);
                    }
                    self.refresh_details();
                    self.reload();
                }
            }
            Message::DropOnTask(parent) => {
                if let Some(id) = self.dragging.take() {
                    match lifecycle::reparent(&mut self.db, id, Some(parent)) {
                        Ok(_) => {
                            self.folds = Folds::default();
                            self.refresh_details();
                            self.reload();
                        }
                        Err(e) => self.fail(e),
                    }
                }
            }
            Message::DConvert(id) => {
                match lifecycle::reparent(&mut self.db, id, None) {
                    Ok(_) => {
                        self.folds = Folds::default();
                        self.refresh_details();
                        self.reload();
                    }
                    Err(e) => self.fail(e),
                }
            }
            Message::DropOut => {
                if let Some(id) = self.dragging.take() {
                    match lifecycle::reparent(&mut self.db, id, None) {
                        Ok(_) => {
                            self.refresh_details();
                            self.reload();
                        }
                        Err(e) => self.fail(e),
                    }
                }
            }
            Message::DropInSection(section) => {
                // A section dropped on another takes its place.
                if let Some(moved) = self.dragging_section.take() {
                    if let Some(target) = section {
                        let place = self
                            .epic_sections
                            .values()
                            .find_map(|v| v.iter().position(|s| s.section.id == target));
                        if let Some(at) = place
                            && target != moved
                            && let Err(e) = lifecycle::move_section(&mut self.db, moved, at)
                        {
                            self.fail(e);
                        }
                        self.reload();
                    }
                    return Task::none();
                }
                if let Some(id) = self.dragging.take() {
                    // Dropped on a section it stands alone there, and its subtasks go with it.
                    if let Err(e) = lifecycle::reparent(&mut self.db, id, None) {
                        return self.fail_with(e);
                    }
                    let mut ids = vec![id];
                    match lifecycle::descendants(&self.db, id) {
                        Ok(d) => ids.extend(d.into_iter().map(|t| t.id)),
                        Err(e) => return self.fail_with(e),
                    }
                    for id in ids {
                        let edit = Edit {
                            section: Some(section),
                            ..Edit::default()
                        };
                        if let Err(e) = lifecycle::edit(&mut self.db, id, &edit) {
                            self.fail(e);
                            break;
                        }
                    }
                    self.refresh_details();
                    self.reload();
                }
            }
            Message::DropOn(to) => {
                if let Some(id) = self.dragging.take() {
                    let from = self
                        .board
                        .iter()
                        .find(|t| t.id == id)
                        .and_then(|t| self.statuses.resolve(t.status))
                        .map(|s| s.id);
                    if from != Some(to) {
                        if let Err(e) = lifecycle::set_status(&mut self.db, id, to) {
                            self.fail(e);
                        }
                        self.refresh_details();
                        self.reload();
                    }
                }
            }
            Message::OpenFilter => self.filter_open = true,
            Message::CloseFilter => self.filter_open = false,
            Message::ClearFilters => {
                self.clear_filters();
                self.reload();
            }
            Message::Archived => {
                self.scope = Scope::Archived;
                self.epic_filter = None;
                self.no_epic = false;
                self.drawer_open = false;
                self.reload();
            }
            Message::Unassigned => {
                self.scope = Scope::All;
                self.epic_filter = None;
                self.no_epic = true;
                self.drawer_open = false;
                self.reload();
            }
            Message::DArchive(on) => {
                if let Some(d) = &self.details
                    && !d.is_new()
                    && matches!(d.subject, Subject::Task(_))
                {
                    let id = d.id();
                    if let Err(e) = lifecycle::set_archived(&mut self.db, id, on) {
                        self.fail(e);
                    } else if on {
                        // It left the list: so does its page.
                        self.details = None;
                    }
                    self.refresh_details();
                    self.reload();
                }
            }
            Message::SyncVault => self.vault_sync(true),
            Message::FocusGained => {
                if self.vault.is_some() {
                    self.vault_sync(false);
                }
            }
            Message::ChooseVault => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Choose the folder for your tasks")
                            .pick_folder()
                            .await
                            .map(|f| f.path().to_path_buf())
                    },
                    Message::VaultChosen,
                );
            }
            Message::VaultChosen(dir) => {
                if let Some(d) = dir {
                    self.set_vault(Some(d));
                }
            }
            Message::UseDefaultVault => {
                self.set_vault(Some(paths::documents_folder(&todo_schema::META)));
            }
            Message::VaultOff => {
                self.set_vault(None);
                self.toast = Some("The tasks are kept in the database alone; the files stay".into());
            }
            Message::OpenVault => {
                if let Some(v) = &self.vault {
                    reveal(v.dir());
                }
            }
            Message::ShowFile => {
                if let (Some(v), Some(d)) = (&self.vault, &self.details)
                    && let Some(rel) = self.vault_known.get(&d.id())
                {
                    let file = v.dir().join(rel);
                    reveal(file.parent().unwrap_or(v.dir()));
                }
            }
            Message::AddFiles => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Add Markdown files")
                            .add_filter("Markdown", &["md", "markdown"])
                            .pick_files()
                            .await
                            .map(|v| v.into_iter().map(|f| f.path().to_path_buf()).collect())
                            .unwrap_or_default()
                    },
                    Message::FilesPicked,
                );
            }
            Message::FilesPicked(files) => {
                if let Some(v) = self.vault.clone() {
                    let mut linked = 0;
                    for f in &files {
                        match v.add_file(&self.db, f, self.epic_filter) {
                            Ok((_, l)) => linked += usize::from(l),
                            Err(e) => self.fail(e),
                        }
                    }
                    self.vault_sync(false);
                    if !files.is_empty() {
                        self.toast = Some(format!(
                            "Added {} file{}{}",
                            files.len(),
                            if files.len() == 1 { "" } else { "s" },
                            if linked > 0 { " (some as links)" } else { "" }
                        ));
                    }
                }
            }
            Message::SetEpic(id) => {
                self.scope = Scope::All;
                self.no_epic = false;
                self.epic_filter = id;
                self.drawer_open = false;
                self.reload();
            }
            Message::TagGroups(event) => {
                self.tag_groups.apply(event);
                self.reload();
            }
            Message::OpenPrefs => {
                self.sticky.refresh(&todo_schema::STICKY);
                self.prefs.open_first_page();
            }
            Message::PrefStickyLink(on) => {
                if let Err(e) = self.sticky.set(&todo_schema::STICKY, on) {
                    self.fail(e);
                }
            }
            Message::DPin(on) => {
                if let Some(d) = &self.details
                    && !d.is_new()
                {
                    if let Err(e) = lifecycle::set_pinned(&mut self.db, d.id(), on) {
                        self.fail(e);
                    }
                    self.refresh_details();
                    self.reload();
                }
            }
            Message::DSticky(on) => {
                let Some(d) = &self.details else {
                    return Task::none();
                };
                if d.is_new() || !matches!(d.subject, Subject::Task(_)) {
                    return Task::none();
                }
                let id = d.id();
                if let Err(e) = organiser_sticky::flag::set_sticky(&mut self.db, id, on) {
                    self.fail(e);
                } else if on && let Err(e) = self.sticky.open_sticky() {
                    self.fail(e);
                }
                self.refresh_details();
                self.reload();
            }
            Message::ClosePrefs => self.prefs.open = false,
            Message::Pref(m) => match self.prefs.update(m) {
                Ok(Changed::Theme(t)) => self.mode = Mode::from(t),
                Ok(Changed::Palette | Changed::Nothing) => {}
                Err(e) => self.fail(e),
            },
            Message::PrefStartView(v) => {
                self.prefs.settings.todo.start_view = v;
                if let Err(e) = self.prefs.save() {
                    self.fail(e);
                }
            }
            Message::PrefShowClosed(on) => {
                self.prefs.settings.todo.show_closed = on;
                if let Err(e) = self.prefs.save() {
                    self.fail(e);
                }
            }
            Message::PrefShowSubtasks(on) => {
                self.prefs.settings.todo.show_subtasks = on;
                if let Err(e) = self.prefs.save() {
                    self.fail(e);
                }
            }
            Message::PrefDetailsDrawer(on) => {
                self.prefs.settings.todo.details_in_drawer = on;
                if let Err(e) = self.prefs.save() {
                    self.fail(e);
                }
            }
            Message::TagQuery(q) => self.tag_query = q,
            Message::Fields(f) => {
                self.filters = f;
                self.reload();
            }
            Message::Tags(m) => {
                if let Err(e) = self.tag_manager.update(&mut self.db, m) {
                    self.fail(e);
                }
                self.refresh_details();
                self.reload();
            }
            Message::EditEpic(id) => {
                self.filter_open = false;
                self.open(id);
            }
            Message::NewEpic => match lifecycle::create_epic(&mut self.db, "New epic") {
                Ok(e) => {
                    self.filter_open = false;
                    self.epic_filter = Some(e.id);
                    self.reload();
                    self.open(e.id);
                }
                Err(e) => self.fail(e),
            },
            Message::ShowClosed(on) => {
                self.show_closed = on;
                self.reload();
            }
            Message::ToggleSearch => {
                self.search.toggle();
                self.reload();
            }
            Message::SearchInput(s) => {
                self.search.input(s);
                self.reload();
            }
            Message::Escape => {
                if self.tag_manager.leave_editor() {
                    // Out of a tag's editor first, back to the list.
                } else if self.prefs.leave_page() {
                    // Back from a page to the list of pages (a narrow window).
                } else if self.prefs.open {
                    self.prefs.open = false;
                } else if self.confirm_epic_delete.is_some() {
                    self.confirm_epic_delete = None;
                } else if self.filter_open {
                    self.filter_open = false;
                } else if self.drawer_open {
                    self.drawer_open = false;
                } else if self.search.is_open() {
                    self.search.close();
                    self.reload();
                } else if self.details.as_ref().is_some_and(|d| d.adding_tag) {
                    self.with_details(|d| {
                        d.adding_tag = false;
                        d.tag_text.clear();
                    });
                } else {
                    self.details = None;
                }
            }
            Message::New => {
                let status = self.statuses.first_open().map(|s| s.id);
                self.details = Some(Details {
                    subject: Subject::Task(TodoTask {
                        id: 0,
                        parent_id: None,
                        // Under the epic being looked at, as a task added there would be.
                        epic: self.epic_filter,
                        section: None,
                        title: String::new(),
                        notes: String::new(),
                        status,
                        priority: Priority::default(),
                        start: None,
                        due: None,
                        tags: Vec::new(),
                        position: 0,
                        archived: false,
                        sticky: false,
                        pinned: false,
                        created: 0,
                        updated: 0,
                    }),
                    title: String::new(),
                    notes: text_editor::Content::new(),
                    preview: false,
                    doc: Vec::new(),
                    start: String::new(),
                    due: String::new(),
                    adding_tag: false,
                    tag_text: String::new(),
                    sub_text: String::new(),
                    subtasks: Vec::new(),
                    error: None,
                });
            }
            Message::DCreate => {
                let Some(d) = &self.details else {
                    return Task::none();
                };
                let Subject::Task(t) = &d.subject else {
                    return Task::none();
                };
                if !d.is_new() {
                    return Task::none();
                }
                let dates = (self.date(&d.start), self.date(&d.due));
                let problem = if d.title.trim().is_empty() {
                    Some("Give the task a title first.".to_string())
                } else {
                    match &dates {
                        (Err(m), _) | (_, Err(m)) => Some(m.clone()),
                        _ => None,
                    }
                };
                if let Some(m) = problem {
                    self.with_details(|d| d.error = Some(m));
                    return Task::none();
                }
                let (Ok(start), Ok(due)) = dates else {
                    return Task::none();
                };
                let new = NewTask {
                    title: d.title.trim().to_string(),
                    notes: d.notes.text().trim_end().to_string(),
                    epic: t.epic,
                    section: t.section,
                    pinned: t.pinned,
                    start,
                    due,
                    priority: Some(t.priority),
                    status: t.status,
                    tags: t.tags.clone(),
                    ..NewTask::default()
                };
                match lifecycle::create(&mut self.db, &new) {
                    Ok(created) => {
                        self.reload();
                        self.open(created.id);
                    }
                    Err(e) => {
                        self.with_details(|d| d.error = Some(e.to_string()));
                    }
                }
            }
            Message::Toggle(id) => {
                // A finished task reopens into the first open status, an open one is finished.
                let target = self.db.get(id).map(|i| {
                    let st = &self.statuses;
                    if st.is_open(TodoTask::from(&i).status) {
                        st.first_closed().map(|s| s.id)
                    } else {
                        st.first_open().map(|s| s.id)
                    }
                });
                match target {
                    Ok(Some(to)) => {
                        if let Err(e) = lifecycle::set_status(&mut self.db, id, to) {
                            self.fail(e);
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        self.fail(e);
                        return Task::none();
                    }
                }
                self.refresh_details();
                self.reload();
            }
            Message::Fold(id) => self.folds.toggle(id),
            Message::Select(id) => self.open(id),
            Message::CloseDetails => self.details = None,
            Message::DTitle(s) => {
                if self.with_details(|d| d.title = s) {
                    self.commit();
                }
            }
            Message::DNotes(a) => {
                let edit = a.is_edit();
                if self.with_details(|d| d.notes.perform(a)) && edit {
                    self.commit();
                }
            }
            Message::DStart(s) => {
                if self.with_details(|d| d.start = s) {
                    self.commit();
                }
            }
            Message::DDue(s) => {
                if self.with_details(|d| d.due = s) {
                    self.commit();
                }
            }
            Message::DStatus(s) => {
                let Some(d) = &self.details else {
                    return Task::none();
                };
                let id = d.id();
                let result = match d.subject {
                    Subject::Task(_) => lifecycle::set_status(&mut self.db, id, s.id).map(|_| ()),
                    // Only a task has a status: an epic has its progress.
                    Subject::Epic(_) | Subject::Section(_) => Ok(()),
                };
                if let Err(e) = result {
                    self.fail(e);
                }
                self.refresh_details();
                self.reload();
            }
            Message::DPriority(p) => {
                if let Some(Details {
                    subject: Subject::Task(t),
                    ..
                }) = &self.details
                {
                    let id = t.id;
                    if let Err(e) = lifecycle::edit(
                        &mut self.db,
                        id,
                        &Edit {
                            priority: Some(p),
                            ..Edit::default()
                        },
                    ) {
                        self.fail(e);
                    }
                    self.refresh_details();
                    self.reload();
                }
            }
            Message::DEpic(choice) => {
                if let Some(Details {
                    subject: Subject::Task(t),
                    ..
                }) = &self.details
                {
                    let id = t.id;
                    if let Err(e) = lifecycle::edit(
                        &mut self.db,
                        id,
                        &Edit {
                            epic: Some(choice.id),
                            ..Edit::default()
                        },
                    ) {
                        self.fail(e);
                    }
                    self.refresh_details();
                    self.reload();
                }
            }
            Message::DSection(section) => {
                if let Some(Details {
                    subject: Subject::Task(t),
                    ..
                }) = &self.details
                {
                    let id = t.id;
                    if let Err(e) = lifecycle::edit(
                        &mut self.db,
                        id,
                        &Edit {
                            section: Some(section),
                            ..Edit::default()
                        },
                    ) {
                        self.fail(e);
                    }
                    self.refresh_details();
                    self.reload();
                }
            }
            Message::DColorText(text) => {
                let parsed = Hsva::parse(&text).map(|h| {
                    let [r, g, b] = h.to_rgb8();
                    format!("#{r:02x}{g:02x}{b:02x}")
                });
                if let Some(d) = &self.details {
                    self.status_text.insert(d.id(), text);
                }
                if let Some(hex) = parsed {
                    self.set_color(hex);
                }
            }
            Message::DIcon(name) => {
                if let Some(Details {
                    subject: Subject::Epic(e),
                    ..
                }) = &self.details
                {
                    let id = e.id;
                    let edit = Edit {
                        icon: Some(name.map(String::from)),
                        ..Edit::default()
                    };
                    if let Err(e) = lifecycle::edit_epic(&mut self.db, id, &edit) {
                        self.fail(e);
                    }
                    self.refresh_details();
                    self.reload();
                }
            }
            Message::DColor(h) => {
                let [r, g, b] = h.to_rgb8();
                let hex = format!("#{r:02x}{g:02x}{b:02x}");
                if let Some(d) = &self.details {
                    self.status_text.insert(d.id(), hex.clone());
                }
                self.set_color(hex);
            }
            Message::NewSection(epic) => match lifecycle::create_section(&mut self.db, epic, "New section") {
                Ok(sec) => {
                    self.reload();
                    self.open(sec.id);
                }
                Err(e) => self.fail(e),
            },
            Message::SectionFold(id) => {
                if !self.folded_sections.remove(&id) {
                    self.folded_sections.insert(id);
                }
            }
            Message::NewInSection(id) => {
                let _ = self.update(Message::New);
                if let Some(Details {
                    subject: Subject::Task(t),
                    ..
                }) = &mut self.details
                {
                    t.section = Some(id);
                }
            }
            Message::DTagStart => {
                self.with_details(|d| d.adding_tag = true);
            }
            Message::DTagText(s) => {
                self.with_details(|d| d.tag_text = s);
            }
            Message::DTagAdd(name) => {
                let name = name.trim().trim_start_matches(['#', '@']).to_string();
                // An empty Enter ends adding; a name is added and the entry stays for the next.
                self.with_details(|d| {
                    d.tag_text.clear();
                    d.adding_tag = !name.is_empty();
                });
                if !name.is_empty() {
                    self.set_tags(|tags| tags.push(name));
                }
            }
            Message::DTagRemove(name) => self.set_tags(|tags| tags.retain(|t| *t != name)),
            Message::DSubText(s) => {
                self.with_details(|d| d.sub_text = s);
            }
            Message::DSubAdd => {
                let Some(Details {
                    subject: Subject::Task(t),
                    sub_text,
                    ..
                }) = &self.details
                else {
                    return Task::none();
                };
                let title = sub_text.trim().to_string();
                if title.is_empty() {
                    return Task::none();
                }
                let new = NewTask {
                    title,
                    parent: Some(t.id),
                    epic: t.epic,
                    ..NewTask::default()
                };
                match lifecycle::create(&mut self.db, &new) {
                    Ok(_) => {
                        self.with_details(|d| d.sub_text.clear());
                        self.folds = Folds::default();
                        self.refresh_details();
                        self.reload();
                    }
                    Err(e) => self.fail(e),
                }
            }
            Message::DDelete => {
                let Some(d) = &self.details else {
                    return Task::none();
                };
                let id = d.id();
                // An epic that has tasks asks what becomes of them first.
                if matches!(d.subject, Subject::Epic(_))
                    && self.epics.iter().any(|s| s.epic.id == id && s.total > 0)
                {
                    self.confirm_epic_delete = Some(id);
                    return Task::none();
                }
                let result = match d.subject {
                    Subject::Task(_) => self
                        .db
                        .delete(id)
                        .map(|n| format!("Deleted {n} task{}", if n == 1 { "" } else { "s" })),
                    Subject::Epic(_) => lifecycle::delete_epic(&mut self.db, id)
                        .map(|()| "Deleted epic".to_string()),
                    Subject::Section(_) => lifecycle::delete_section(&mut self.db, id)
                        .map(|()| "Deleted section".to_string()),
                };
                match result {
                    Ok(m) => self.toast = Some(m),
                    Err(e) => self.fail(e),
                }
                if self.epic_filter == Some(id) {
                    self.epic_filter = None;
                }
                self.details = None;
                self.reload();
            }
            Message::EpicDeleteCancel => self.confirm_epic_delete = None,
            Message::EpicDeleteKeep | Message::EpicDeleteAll => {
                let all = matches!(message, Message::EpicDeleteAll);
                if let Some(id) = self.confirm_epic_delete.take() {
                    let result = if all {
                        lifecycle::delete_epic_and_tasks(&mut self.db, id).map(|n| {
                            format!("Deleted the epic and {n} task{}", if n == 1 { "" } else { "s" })
                        })
                    } else {
                        lifecycle::delete_epic(&mut self.db, id)
                            .map(|()| "Deleted the epic; its tasks stay".to_string())
                    };
                    match result {
                        Ok(m) => self.toast = Some(m),
                        Err(e) => self.fail(e),
                    }
                    if self.epic_filter == Some(id) {
                        self.epic_filter = None;
                    }
                    self.details = None;
                    self.reload();
                }
            }
            Message::DPreview(on) => {
                self.with_details(|d| d.preview = on);
            }
            Message::DTask(n, on) => {
                let source = self.details.as_ref().map(|d| d.notes.text());
                if let Some(new) = source.and_then(|s| organiser_markdown::tasks::set(&s, n, on)) {
                    self.with_details(|d| d.notes = text_editor::Content::with_text(&new));
                    self.commit();
                }
            }
            Message::OpenLink(url) => {
                if let Err(e) = organiser_ui::shell::open_link(&url) {
                    self.toast = Some(e);
                }
            }
            Message::StatusNew => {
                let name = (1..)
                    .map(|n| format!("Status {n}"))
                    .find(|n| self.statuses.by_name(n).is_none())
                    .unwrap_or_default();
                match statuses::create(&mut self.db, &name, Some("#9aa0a6"), false) {
                    Ok(_) => self.reload(),
                    Err(e) => self.fail(e),
                }
            }
            Message::StatusName(id, name) => {
                // An empty or taken name is not saved; the entry keeps what was typed until it is.
                self.status_names.insert(id, name.clone());
                if !name.trim().is_empty() {
                    let e = StatusEdit {
                        name: Some(name),
                        ..StatusEdit::default()
                    };
                    if let Err(e) = statuses::edit(&mut self.db, id, &e) {
                        self.fail(e);
                    }
                    self.reload();
                }
            }
            Message::StatusColorText(id, text) => {
                let parsed = Hsva::parse(&text).map(|h| {
                    let [r, g, b] = h.to_rgb8();
                    format!("#{r:02x}{g:02x}{b:02x}")
                });
                self.status_text.insert(id, text);
                if let Some(hex) = parsed {
                    self.set_status_color(id, hex);
                }
            }
            Message::StatusColor(id, h) => {
                let [r, g, b] = h.to_rgb8();
                let hex = format!("#{r:02x}{g:02x}{b:02x}");
                self.status_text.insert(id, hex.clone());
                self.set_status_color(id, hex);
            }
            Message::StatusClosed(id, closed) => {
                let e = StatusEdit {
                    closed: Some(closed),
                    ..StatusEdit::default()
                };
                if let Err(e) = statuses::edit(&mut self.db, id, &e) {
                    self.fail(e);
                }
                self.reload();
            }
            Message::StatusMove(id, step) => {
                if let Err(e) = statuses::move_by(&mut self.db, id, step) {
                    self.fail(e);
                }
                self.reload();
            }
            Message::StatusDelete(id) => {
                match statuses::delete(&mut self.db, id) {
                    Ok(()) => self.status_text.remove(&id).map_or((), |_| ()),
                    Err(e) => self.fail(e),
                }
                self.refresh_details();
                self.reload();
            }
            Message::DismissToast => self.toast = None,
            Message::Captured => return iced::exit(),
        }
        Task::none()
    }
}

/// Show a directory in the file manager.
fn reveal(dir: &std::path::Path) {
    let _ = std::process::Command::new("xdg-open")
        .arg(dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
