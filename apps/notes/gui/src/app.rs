//! State, messages and update. Every change goes through `notes-schema` and the engine; the view
//! never touches the database.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use iced::widget::text_editor;
use iced::{Color as IcedColor, Subscription, Task};
use notes_schema::color::Color;
use notes_schema::lifecycle::{self, NewNote};
use notes_schema::md_tasks;
use notes_schema::note::{Edit, Note};
use notes_schema::views::{self, Filter, Scope};
use organiser_core::paths;
use organiser_engine::{Db, Filters, Tag};
use organiser_ui::components::tag_query::{count as tag_count, engine_filter, retain_existing};
use organiser_ui::iad::primitives::color_picker::Hsva;
use organiser_ui::iad::primitives::rich_text as rt;
use organiser_ui::iad::primitives::tag_groups::{GroupEvent, TagGroups};
use organiser_ui::iad::{self, Scheme};
use organiser_ui::primitives::scheme::Mode;
use organiser_ui::shell;
use organiser_ui::primitives::search::Search;
use organiser_ui::widgets::preferences::{Changed, PrefMsg, Preferences};
use organiser_ui::widgets::sticky_link::StickyLink;
use organiser_ui::widgets::tag_manager::{TagManager, TagMsg};

#[derive(Debug, Clone)]
pub enum Message {
    SystemTheme(iad::utils::ThemeSettings),
    Window(iad::windows::Action),
    Quit,
    /// Open or close the sidebar drawer when the window is too narrow to show it.
    Drawer(bool),
    SetScope(Scope),
    /// The tag filter's groups changed.
    TagGroups(GroupEvent<String>),
    OpenFilter,
    CloseFilter,
    ClearFilters,
    TagQuery(String),
    /// The filters the schema offers changed.
    Fields(Filters),
    Tags(TagMsg),
    OpenPrefs,
    ClosePrefs,
    Pref(PrefMsg),
    /// Notes' own setting: days a note stays in the trash.
    PrefTrashDays(u32),
    /// Link (true) or unlink this app to the Sticky Notes app.
    PrefStickyLink(bool),
    ToggleSearch,
    SearchInput(String),
    Escape,
    /// Open the edit page on a new note. Nothing is saved until `DCreate`.
    New,
    /// Save the new note being edited.
    DCreate,
    EmptyTrash,
    // The details of the selected note.
    Select(i64),
    CloseDetails,
    DTitle(String),
    DBody(text_editor::Action),
    /// Show the note drawn (true) or its Markdown source (false).
    DPreview(bool),
    /// Task `n` of the drawn note was ticked or cleared.
    DTask(usize, bool),
    /// A link in the drawn note was pressed.
    OpenLink(String),
    /// A colour from a swatch, or `None` for the default card.
    DColor(Option<Color>),
    /// Text typed in the custom colour entry.
    DColorText(String),
    /// A colour dragged in the custom picker.
    DColorPick(Hsva),
    DPin(bool),
    /// Show the note as a desktop sticky (true) or stop.
    DSticky(bool),
    DArchive(bool),
    DTrash,
    DRestore,
    DPurge,
    DTagStart,
    DTagText(String),
    /// Add a tag; an empty name stops adding.
    DTagAdd(String),
    DTagRemove(String),
    DismissToast,
    /// A development screenshot was saved (or failed); the app then quits.
    Captured,
}

/// The selected note. The text fields hold what the person typed; each edit is saved at once, so
/// there is no Save button and nothing to lose.
pub struct Details {
    pub note: Note,
    pub title: String,
    pub body: text_editor::Content,
    /// Draw the body instead of editing its source.
    pub preview: bool,
    /// The body drawn, kept so a redraw does not parse it again.
    pub doc: Vec<rt::Block>,
    /// What the custom colour entry shows, kept so typing is not interrupted.
    pub color_text: String,
    pub adding_tag: bool,
    pub tag_text: String,
    pub error: Option<String>,
}

impl Details {
    /// Whether this is a note being written, not yet saved: the edit page doubles as the page
    /// that creates one, and a draft has no id.
    pub fn is_new(&self) -> bool {
        self.note.id == 0
    }
}

pub struct App {
    db: Db,
    pub scope: Scope,
    /// Notes must have all of these tags.
    /// The picked tags: groups that are alternatives, every tag of one needed.
    pub tag_groups: TagGroups<String>,
    /// What was chosen in the filters the schema offers.
    pub filters: Filters,
    pub filter_open: bool,
    /// What was typed to find a tag in the filter.
    pub tag_query: String,
    pub tag_manager: TagManager,
    /// The shared settings and the Preferences sheet over them.
    pub prefs: Preferences,
    /// Whether Sticky Notes is installed and this app is linked to it.
    pub sticky: StickyLink,
    pub notes: Vec<Note>,
    /// Each listed note's body drawn, by note id.
    pub docs: HashMap<i64, Vec<rt::Block>>,
    pub search: Search,
    pub tags: Vec<Tag>,
    pub tag_names: Vec<String>,
    pub drawer_open: bool,
    pub details: Option<Details>,
    pub toast: Option<String>,
    pub desktop_dark: bool,
    pub mode: Mode,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// A note colour as an iced colour, for painting.
pub fn paint(c: Color) -> IcedColor {
    IcedColor::from_rgb8(c.r, c.g, c.b)
}

fn hsva(c: Option<Color>) -> Hsva {
    let c = c.unwrap_or(Color::rgb(0x6a, 0xa5, 0xe8));
    Hsva::from_rgb8(c.r, c.g, c.b)
}

impl App {
    pub fn boot() -> (Self, Task<Message>) {
        let args: Vec<String> = std::env::args().collect();
        let path = args
            .iter()
            .position(|a| a == "--db")
            .and_then(|i| args.get(i + 1))
            .map_or_else(
                || paths::db_path(&notes_schema::META),
                std::path::PathBuf::from,
            );
        let mut db = Db::open(&path, &notes_schema::SCHEMA).unwrap_or_else(|e| {
            eprintln!("error: cannot open {}: {e}", path.display());
            std::process::exit(1);
        });
        let prefs = Preferences::load();
        let mode = Mode::from(prefs.settings.appearance.theme);
        // Housekeeping: notes past their time in the trash go for good.
        if let Err(e) = lifecycle::purge_expired(&mut db, now(), prefs.settings.notes.trash_days) {
            eprintln!("warning: cannot empty the old trash: {e}");
        }
        let mut app = Self {
            db,
            scope: Scope::Notes,
            tag_groups: TagGroups::new(),
            filters: Filters::default(),
            filter_open: std::env::var_os("NOTES_GUI_FILTER").is_some(),
            tag_query: String::new(),
            tag_manager: TagManager::default(),
            prefs,
            sticky: StickyLink::load(&notes_schema::STICKY),
            notes: Vec::new(),
            docs: HashMap::new(),
            search: Search::default(),
            tags: Vec::new(),
            tag_names: Vec::new(),
            drawer_open: false,
            details: None,
            toast: None,
            desktop_dark: false,
            mode,
        };
        // Development switches, for screenshots of states that need a click to reach.
        match std::env::var("NOTES_GUI_SCOPE").as_deref() {
            Ok("archive") => app.scope = Scope::Archive,
            Ok("trash") => app.scope = Scope::Trash,
            _ => {}
        }
        if let Ok(q) = std::env::var("NOTES_GUI_SEARCH") {
            app.search.toggle();
            app.search.input(q);
        }
        if let Ok(t) = std::env::var("NOTES_GUI_TAG") {
            app.tag_groups =
                TagGroups::from_parts(vec![t.split(',').map(String::from).collect()], 0);
        }
        app.reload();
        if let Some(id) = std::env::var("NOTES_GUI_SELECT")
            .ok()
            .and_then(|v| v.parse().ok())
        {
            app.open(id);
        }
        if std::env::var_os("NOTES_GUI_NEW").is_some() || shell::wants_new(&notes_schema::META) {
            let _ = app.update(Message::New);
        }
        if let Ok(name) = std::env::var("NOTES_GUI_TAG_EDIT") {
            let _ = app.update(Message::Tags(TagMsg::Edit(Some(name))));
        }
        if let Ok(page) = std::env::var("NOTES_GUI_PREFS") {
            app.prefs.open = true;
            // Asking for a page opens it, on a narrow window too.
            app.prefs.page_open = std::env::var_os("NOTES_GUI_PREFS_LIST").is_none();
            app.prefs.page = match page.as_str() {
                "tags" => organiser_ui::widgets::preferences::Page::Tags,
                "app" => organiser_ui::widgets::preferences::Page::App,
                _ => organiser_ui::widgets::preferences::Page::Appearance,
            };
        }
        if std::env::var_os("NOTES_GUI_PALETTE_EDIT").is_some() {
            app.prefs.palette_editing = true;
        }
        match std::env::var("NOTES_GUI_THEME").as_deref() {
            Ok("light") => app.mode = Mode::Light,
            Ok("dark") => app.mode = Mode::Dark,
            _ => {}
        }
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
        (app, Task::batch(tasks))
    }

    pub fn title(&self) -> String {
        "Notes".into()
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
            text: self.search.query().map(String::from),
            tags: engine_filter(&self.tag_groups),
            fields: self.filters.clone(),
            // A search looks in the archive too, so an old note is still found.
            include_archived: self.search.query().is_some(),
        }
    }

    /// Re-read what the page shows from the database.
    fn reload(&mut self) {
        match views::notes(&self.db, &self.filter()) {
            Ok(v) => {
                self.docs = v
                    .iter()
                    .filter(|n| !n.body.is_empty())
                    .map(|n| (n.id, organiser_ui::components::rich::render(&n.body)))
                    .collect();
                self.notes = v;
            }
            Err(e) => self.toast = Some(e.to_string()),
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

    /// How many filters narrow the list: tags and the schema's own.
    pub fn filter_count(&self) -> usize {
        tag_count(&self.tag_groups) + self.filters.count()
    }

    fn fail(&mut self, e: impl std::fmt::Display) {
        self.toast = Some(e.to_string());
    }

    /// The pinned notes, then the rest. Pinning only groups the live notes.
    pub fn pinned(&self) -> (Vec<&Note>, Vec<&Note>) {
        if self.scope == Scope::Notes {
            self.notes.iter().partition(|n| n.pinned)
        } else {
            (Vec::new(), self.notes.iter().collect())
        }
    }

    // ---- details ---------------------------------------------------------------------------

    fn open(&mut self, id: i64) {
        let note = match self.db.get(id) {
            Ok(i) => Note::from(&i),
            Err(e) => return self.fail(e),
        };
        self.details = Some(Details {
            title: note.title.clone(),
            body: text_editor::Content::with_text(&note.body),
            preview: std::env::var_os("NOTES_GUI_PREVIEW").is_some(),
            doc: organiser_ui::components::rich::render(&note.body),
            color_text: note.color.map(|c| c.hex()).unwrap_or_default(),
            note,
            adding_tag: false,
            tag_text: String::new(),
            error: None,
        });
    }

    /// Re-read the selected note (pin, colour, tags) without touching what is typed.
    fn refresh_details(&mut self) {
        let Some(d) = &self.details else { return };
        match self.db.get(d.note.id) {
            Ok(item) => {
                if let Some(d) = &mut self.details {
                    d.note = Note::from(&item);
                }
            }
            Err(_) => self.details = None,
        }
    }

    pub fn color_value(&self) -> Hsva {
        hsva(self.details.as_ref().and_then(|d| d.note.color))
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

    /// Save the typed fields. Called after every edit.
    fn commit(&mut self) {
        let Some(d) = &self.details else { return };
        if d.is_new() {
            // A draft is kept as typed until it is created.
            if let Some(d) = &mut self.details {
                d.doc = organiser_ui::components::rich::render(&d.body.text());
            }
            return;
        }
        let edit = Edit {
            // An emptied title keeps the old one: a note must have a title.
            title: (!d.title.trim().is_empty()).then(|| d.title.clone()),
            body: Some(d.body.text().trim_end().to_string()),
            ..Edit::default()
        };
        let id = d.note.id;
        let result = lifecycle::edit(&mut self.db, id, &edit);
        if let Some(d) = &mut self.details {
            d.error = result.err().map(|e| e.to_string());
            d.doc = organiser_ui::components::rich::render(&d.body.text());
        }
        self.refresh_details();
        self.reload();
    }

    /// Run a change on the selected note, then show the result everywhere.
    fn act(&mut self, f: impl FnOnce(&mut Db, i64) -> organiser_engine::Result<()>) {
        let Some(d) = &self.details else { return };
        if d.is_new() {
            return;
        }
        let id = d.note.id;
        if let Err(e) = f(&mut self.db, id) {
            self.fail(e);
        }
        self.refresh_details();
        self.reload();
    }

    fn set_color(&mut self, c: Option<Color>) {
        if self.details.as_ref().is_some_and(Details::is_new) {
            self.with_details(|d| d.note.color = c);
            self.with_details(|d| d.color_text = c.map(|c| c.hex()).unwrap_or_default());
            return;
        }
        self.act(|db, id| {
            lifecycle::edit(
                db,
                id,
                &Edit {
                    color: Some(c),
                    ..Edit::default()
                },
            )
            .map(|_| ())
        });
        self.with_details(|d| d.color_text = c.map(|c| c.hex()).unwrap_or_default());
    }

    fn set_tags(&mut self, f: impl FnOnce(&mut Vec<String>)) {
        let Some(d) = &self.details else { return };
        let mut tags = d.note.tags.clone();
        f(&mut tags);
        if d.is_new() {
            self.with_details(|d| d.note.tags = tags);
            return;
        }
        let id = d.note.id;
        if let Err(e) = lifecycle::set_tags(&mut self.db, id, &tags)
            && let Some(d) = &mut self.details
        {
            d.error = Some(e.to_string());
        }
        self.refresh_details();
        self.reload();
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        // A draft has nothing to archive, trash, restore or delete yet.
        if matches!(
            message,
            Message::DArchive(_) | Message::DTrash | Message::DRestore | Message::DPurge
        ) && self.details.as_ref().is_some_and(Details::is_new)
        {
            return Task::none();
        }
        match message {
            Message::SystemTheme(s) => self.desktop_dark = s.scheme == Scheme::Dark,
            Message::Window(a) => return iced::window::oldest().and_then(move |id| a.perform(id)),
            Message::Quit => return iced::exit(),
            Message::Drawer(open) => self.drawer_open = open,
            Message::SetScope(s) => {
                self.scope = s;
                self.drawer_open = false;
                self.details = None;
                self.reload();
            }
            Message::TagGroups(event) => {
                self.tag_groups.apply(event);
                self.reload();
            }
            Message::OpenFilter => self.filter_open = true,
            Message::CloseFilter => self.filter_open = false,
            Message::ClearFilters => {
                self.tag_groups = TagGroups::new();
                self.filters.clear();
                self.reload();
            }
            Message::TagQuery(q) => self.tag_query = q,
            Message::Fields(f) => {
                self.filters = f;
                self.reload();
            }
            Message::OpenPrefs => {
                self.sticky.refresh(&notes_schema::STICKY);
                self.prefs.open_first_page();
            }
            Message::ClosePrefs => self.prefs.open = false,
            Message::Pref(m) => match self.prefs.update(m) {
                Ok(Changed::Theme(t)) => self.mode = Mode::from(t),
                Ok(Changed::Palette | Changed::Nothing) => {}
                Err(e) => self.fail(e),
            },
            Message::PrefTrashDays(d) => {
                self.prefs.settings.notes.trash_days = d;
                if let Err(e) = self.prefs.save() {
                    self.fail(e);
                }
            }
            Message::PrefStickyLink(on) => {
                if let Err(e) = self.sticky.set(&notes_schema::STICKY, on) {
                    self.fail(e);
                }
            }
            Message::Tags(m) => {
                if let Err(e) = self.tag_manager.update(&mut self.db, m) {
                    self.fail(e);
                }
                self.refresh_details();
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
                self.details = Some(Details {
                    note: Note {
                        id: 0,
                        title: String::new(),
                        body: String::new(),
                        color: None,
                        pinned: false,
                        sticky: false,
                        archived: false,
                        trashed: None,
                        tags: Vec::new(),
                        created: 0,
                        updated: 0,
                    },
                    title: String::new(),
                    body: text_editor::Content::new(),
                    preview: false,
                    doc: Vec::new(),
                    color_text: String::new(),
                    adding_tag: false,
                    tag_text: String::new(),
                    error: None,
                });
            }
            Message::DCreate => {
                let Some(d) = &self.details else {
                    return Task::none();
                };
                if !d.is_new() {
                    return Task::none();
                }
                if d.title.trim().is_empty() {
                    self.with_details(|d| d.error = Some("Give the note a title first.".into()));
                    return Task::none();
                }
                let new = NewNote {
                    title: d.title.trim().to_string(),
                    body: d.body.text().trim_end().to_string(),
                    color: d.note.color,
                    tags: d.note.tags.clone(),
                };
                let pinned = d.note.pinned;
                let created = lifecycle::create(&mut self.db, &new).and_then(|n| {
                    if pinned {
                        lifecycle::set_pinned(&mut self.db, n.id, true)?;
                    }
                    Ok(n)
                });
                match created {
                    Ok(n) => {
                        // A new note belongs in the notes, whatever the page was showing.
                        self.scope = Scope::Notes;
                        self.reload();
                        self.open(n.id);
                    }
                    Err(e) => {
                        self.with_details(|d| d.error = Some(e.to_string()));
                    }
                }
            }
            Message::EmptyTrash => match lifecycle::empty_trash(&mut self.db) {
                Ok(n) => {
                    self.toast = Some(format!("Deleted {n} note{}", if n == 1 { "" } else { "s" }));
                    self.details = None;
                    self.reload();
                }
                Err(e) => self.fail(e),
            },
            Message::Select(id) => self.open(id),
            Message::CloseDetails => self.details = None,
            Message::DTitle(s) => {
                if self.with_details(|d| d.title = s) {
                    self.commit();
                }
            }
            Message::DBody(a) => {
                let edit = a.is_edit();
                if self.with_details(|d| d.body.perform(a)) && edit {
                    self.commit();
                }
            }
            Message::DPreview(on) => {
                self.with_details(|d| d.preview = on);
            }
            Message::DTask(n, on) => {
                let source = self.details.as_ref().map(|d| d.body.text());
                if let Some(new) = source.and_then(|s| md_tasks::set(&s, n, on)) {
                    self.with_details(|d| d.body = text_editor::Content::with_text(&new));
                    self.commit();
                }
            }
            Message::OpenLink(url) => {
                if let Err(e) = organiser_ui::shell::open_link(&url) {
                    self.toast = Some(e);
                }
            }
            Message::DColor(c) => self.set_color(c),
            Message::DColorText(s) => {
                // A readable colour applies as it is typed; anything else waits for more text.
                let parsed = Hsva::parse(&s).map(|h| {
                    let [r, g, b] = h.to_rgb8();
                    Color::rgb(r, g, b)
                });
                match parsed {
                    Some(c) => {
                        self.set_color(Some(c));
                        self.with_details(|d| d.color_text = s);
                    }
                    None => {
                        self.with_details(|d| d.color_text = s);
                    }
                }
            }
            Message::DColorPick(h) => {
                let [r, g, b] = h.to_rgb8();
                self.set_color(Some(Color::rgb(r, g, b)));
            }
            Message::DPin(on) => {
                if self.details.as_ref().is_some_and(Details::is_new) {
                    self.with_details(|d| d.note.pinned = on);
                } else {
                    self.act(|db, id| lifecycle::set_pinned(db, id, on).map(|_| ()));
                }
            }
            Message::DSticky(on) => {
                if self.details.as_ref().is_some_and(Details::is_new) {
                    return Task::none();
                }
                self.act(|db, id| organiser_sticky::flag::set_sticky(db, id, on).map(|_| ()));
                if on && let Err(e) = self.sticky.open_sticky() {
                    self.fail(e);
                }
            }
            Message::DArchive(on) => {
                self.act(|db, id| lifecycle::set_archived(db, id, on).map(|_| ()));
            }
            Message::DTrash => {
                self.act(|db, id| lifecycle::trash(db, id, now()).map(|_| ()));
                self.toast = Some("Moved to the trash".into());
                self.details = None;
            }
            Message::DRestore => {
                self.act(|db, id| lifecycle::restore(db, id).map(|_| ()));
                self.details = None;
            }
            Message::DPurge => {
                self.act(lifecycle::purge);
                self.details = None;
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
            Message::DismissToast => self.toast = None,
            Message::Captured => return iced::exit(),
        }
        Task::none()
    }
}
