//! State, messages and update. Every change goes through `todo-schema` and the engine; the view
//! never touches the database.

use std::collections::HashMap;

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
use organiser_ui::primitives::search::Search;
use organiser_ui::primitives::text::ellipsize;
use organiser_ui::widgets::preferences::{Changed, PrefMsg, Preferences};
use organiser_ui::widgets::sticky_link::StickyLink;
use organiser_ui::widgets::tag_manager::{TagManager, TagMsg};
use todo_schema::EPIC;
use todo_schema::board::BoardView;
use todo_schema::lifecycle::{self, NewTask};
use todo_schema::priority::Priority;
use todo_schema::quick;
use todo_schema::status::{Status, Statuses};
use todo_schema::statuses::{self, StatusEdit};
use todo_schema::task::{Edit, Epic, EpicSummary, Task as TodoTask};
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
    /// Switch between the list and the kanban board.
    Kanban(bool),
    /// Open or close the board's sidebar when the window is too narrow to show it.
    Drawer(bool),
    /// Move a card to the next column (`1`) or the previous one (`-1`).
    MoveCard(i64, i32),
    OpenFilter,
    CloseFilter,
    ClearFilters,
    ToggleEpic(i64),
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
    /// Link (true) or unlink this app to the Sticky Notes app.
    PrefStickyLink(bool),
    /// Show the selected task as a desktop sticky (true) or stop.
    DSticky(bool),
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
        }
    }

    pub fn tags(&self) -> &[String] {
        match &self.subject {
            Subject::Task(t) => &t.tags,
            Subject::Epic(e) => &e.tags,
        }
    }
}

pub struct App {
    db: Db,
    pub today: NaiveDate,
    pub scope: Scope,
    pub epic_filter: Option<i64>,
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
    pub kanban: bool,
    /// The board's tasks: every status, whatever the list's own filters hide.
    pub board: Vec<TodoTask>,
    /// `(done, total)` subtasks of each board task that has any.
    pub sub_progress: HashMap<i64, (usize, usize)>,
    pub drawer_open: bool,
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
    /// The notes of the board's tasks, drawn, by task id.
    pub docs: HashMap<i64, Vec<rt::Block>>,
    pub search: Search,
    pub forest: Vec<Node<TodoTask>>,
    pub epics: Vec<EpicSummary>,
    pub epic_titles: HashMap<i64, String>,
    pub epic_choices: Vec<EpicChoice>,
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
        let board_by_default = prefs.settings.todo.start_view == StartView::Board;
        let show_closed_by_default = prefs.settings.todo.show_closed;
        let mut app = Self {
            db,
            today: views::today(),
            scope: Scope::All,
            epic_filter: None,
            tag_groups: TagGroups::new(),
            prefs,
            sticky: StickyLink::load(&todo_schema::STICKY),
            filters: Filters::default(),
            tag_manager: TagManager::default(),
            filter_open: false,
            tag_query: String::new(),
            show_closed: show_closed_by_default,
            flat: false,
            // The board is the default view; the list is one toggle away.
            kanban: board_by_default,
            menu_open: false,
            board: Vec::new(),
            sub_progress: HashMap::new(),
            drawer_open: false,
            flat_tasks: Vec::new(),
            task_titles: HashMap::new(),
            statuses: Statuses::default(),
            status_text: HashMap::new(),
            status_names: HashMap::new(),
            docs: HashMap::new(),
            search: Search::default(),
            forest: Vec::new(),
            epics: Vec::new(),
            epic_titles: HashMap::new(),
            epic_choices: Vec::new(),
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
            app.kanban = false;
        }
        if std::env::var_os("TODO_GUI_CURRENT").is_some() {
            app.scope = Scope::Current;
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
        if std::env::var_os("TODO_GUI_NEW").is_some() {
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
        (app, Task::batch(tasks))
    }

    pub fn title(&self) -> String {
        "Todo".into()
    }

    pub fn theme(&self) -> iad::Theme {
        organiser_ui::shell::theme(self.mode.is_dark(self.desktop_dark))
    }

    pub fn subscription(&self) -> Subscription<Message> {
        iad::utils::follow_theme().map(Message::SystemTheme)
    }

    fn filter(&self) -> Filter {
        Filter {
            scope: self.scope,
            show_closed: self.show_closed,
            text: self.search.query().map(String::from),
            // The picked tags are alternatives: a task matches if it has any one of them.
            tags: engine_filter(&self.tag_groups),
            epic: self.epic_filter,
            fields: self.filters.clone(),
            include_archived: false,
        }
    }

    /// Re-read what the lists show from the database.
    fn reload(&mut self) {
        self.today = views::today();
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
        if self.kanban {
            self.reload_board();
        }
        self.task_titles = self.titles_of(&self.forest);
        if let Ok(all) = views::epics(&self.db, true) {
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
                    .filter(|s| self.statuses.is_open(s.epic.status))
                    .map(|s| EpicChoice {
                        id: Some(s.epic.id),
                        title: s.epic.title.clone(),
                    }),
            )
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

    /// The board's tasks and how far along each one's subtasks are. The board shows every status
    /// (its views choose the columns), so it asks for closed tasks whatever the list is hiding.
    fn reload_board(&mut self) {
        // The board shows every status; current tasks narrow it like any other filter.
        let f = Filter {
            show_closed: true,
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
                self.docs = tasks
                    .iter()
                    .filter(|t| !t.notes.is_empty())
                    .map(|t| (t.id, organiser_ui::components::rich::render(&t.notes)))
                    .collect();
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

    fn clear_filters(&mut self) {
        self.epic_filter = None;
        self.tag_groups = TagGroups::new();
        self.filters.clear();
    }

    /// How many filters (an epic, tags, the schema's own) narrow the list.
    pub fn filter_count(&self) -> usize {
        usize::from(self.epic_filter.is_some()) + tag_count(&self.tag_groups) + self.filters.count()
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
                        t.epic = c.id;
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
                if on {
                    // The sidebar is one choice: current tasks, or an epic, or all.
                    self.epic_filter = None;
                    self.drawer_open = false;
                }
                self.reload();
            }
            Message::Flat(on) => self.flat = on,
            Message::Kanban(on) => {
                self.kanban = on;
                self.drawer_open = false;
                self.reload();
            }
            Message::Drawer(open) => self.drawer_open = open,
            Message::MoveCard(id, step) => {
                let current = self
                    .board
                    .iter()
                    .find(|t| t.id == id)
                    .and_then(|t| self.statuses.resolve(t.status))
                    .map(|s| s.id);
                let view = self.board_view();
                if let Some(to) = current.and_then(|s| view.neighbor(&self.statuses, s, step)) {
                    if let Err(e) = lifecycle::set_status(&mut self.db, id, to) {
                        self.fail(e);
                    }
                    self.refresh_details();
                    self.reload();
                }
            }
            Message::OpenFilter => self.filter_open = true,
            Message::CloseFilter => self.filter_open = false,
            Message::ClearFilters => {
                self.clear_filters();
                self.reload();
            }
            Message::SetEpic(id) => {
                self.scope = Scope::All;
                self.epic_filter = id;
                self.drawer_open = false;
                self.reload();
            }
            Message::ToggleEpic(id) => {
                self.epic_filter = if self.epic_filter == Some(id) {
                    None
                } else {
                    Some(id)
                };
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
                    Subject::Epic(_) => {
                        lifecycle::set_epic_status(&mut self.db, id, s.id).map(|_| ())
                    }
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
                let result = match d.subject {
                    Subject::Task(_) => self
                        .db
                        .delete(id)
                        .map(|n| format!("Deleted {n} task{}", if n == 1 { "" } else { "s" })),
                    Subject::Epic(_) => lifecycle::delete_epic(&mut self.db, id)
                        .map(|()| "Deleted epic".to_string()),
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
