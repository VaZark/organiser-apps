//! State, messages and update. Every change goes through `faves-schema` and the engine; the view
//! never touches the database.

use faves_schema::lifecycle::{self, NewEntry};
use faves_schema::model::{Entry, EntryEdit, List, ListEdit, ListSummary};
use faves_schema::status::Status;
use faves_schema::views::{self, Filter, Sort};
use iced::widget::text_editor;
use iced::{Subscription, Task};
use organiser_core::paths;
use organiser_engine::{Db, Tag, TagFilter};
use organiser_ui::iad::primitives::tag_groups::{GroupEvent, TagGroups};
use organiser_ui::iad::{self, Scheme};
use organiser_ui::primitives::scheme::Mode;
use organiser_ui::primitives::search::Search;
use organiser_ui::primitives::text::ellipsize;

/// What the content shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// The favourites of every list.
    Favorites,
    List(i64),
}

#[derive(Debug, Clone)]
pub enum Message {
    SystemTheme(iad::utils::ThemeSettings),
    Window(iad::windows::Action),
    Quit,
    Show(View),
    NewList,
    /// Cards, or one compact line per entry.
    Compact(bool),
    OpenLists,
    CloseLists,
    OpenAdd,
    CloseAdd,
    // The filter, opened from the search.
    OpenFilter,
    CloseFilter,
    ClearFilters,
    FavOnly(bool),
    /// Entries with at least this many stars, or any.
    MinRating(Option<u8>),
    TagGroups(GroupEvent<String>),
    TagQuery(String),
    ToggleSearch,
    SearchInput(String),
    Escape,
    QuickText(String),
    QuickSubmit,
    Sort(SortChoice),
    /// Press an entry's heart on its card.
    Fave(i64),
    // The selected entry.
    Open(i64),
    CloseDetails,
    DTitle(String),
    DNotes(text_editor::Action),
    DFavorite(bool),
    /// A star was pressed: that rating, or none.
    DRating(Option<u8>),
    DStatus(StatusChoice),
    DMove(ListChoice),
    DTagStart,
    DTagText(String),
    DTagAdd(String),
    DTagRemove(String),
    DDelete,
    // The settings of the current list.
    OpenSettings,
    CloseSettings,
    SetListTitle(String),
    SetRated(bool),
    SetTracked(bool),
    DeleteList,
    DismissToast,
    Captured,
}

/// A sort, for the dropdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortChoice(pub Sort);

impl SortChoice {
    pub const ALL: [SortChoice; 4] = [
        SortChoice(Sort::Manual),
        SortChoice(Sort::Title),
        SortChoice(Sort::Rating),
        SortChoice(Sort::Recent),
    ];
}

impl std::fmt::Display for SortChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.0 {
            Sort::Manual => "My order",
            Sort::Title => "Title",
            Sort::Rating => "Best rated",
            Sort::Recent => "Newest",
        })
    }
}

/// A status, or none, for the dropdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusChoice(pub Option<Status>);

impl StatusChoice {
    pub const ALL: [StatusChoice; 4] = [
        StatusChoice(None),
        StatusChoice(Some(Status::Planned)),
        StatusChoice(Some(Status::Active)),
        StatusChoice(Some(Status::Done)),
    ];
}

impl std::fmt::Display for StatusChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.map_or("No status", Status::label))
    }
}

/// A list, for the "move to" dropdown.
#[derive(Debug, Clone, PartialEq)]
pub struct ListChoice {
    pub id: i64,
    pub title: String,
}

impl std::fmt::Display for ListChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&ellipsize(&self.title, 30))
    }
}

/// The selected entry. The text fields hold what was typed; each edit is saved at once.
pub struct Details {
    pub entry: Entry,
    pub title: String,
    pub notes: text_editor::Content,
    pub adding_tag: bool,
    pub tag_text: String,
    pub error: Option<String>,
}

pub struct App {
    db: Db,
    pub view: View,
    pub lists: Vec<ListSummary>,
    pub list_choices: Vec<ListChoice>,
    pub entries: Vec<Entry>,
    pub tags: Vec<Tag>,
    pub tag_names: Vec<String>,
    pub sort: Sort,
    pub search: Search,
    pub quick: String,
    pub details: Option<Details>,
    pub settings_open: bool,
    /// One compact line per entry instead of cards.
    pub compact: bool,
    /// Development only: draw the sort popover open (for screenshots).
    pub sort_open: bool,
    /// Development only: draw the header menu open (for screenshots).
    pub menu_open: bool,
    /// Development only: with the menu open, start on this page (1 is Sort).
    pub menu_page: usize,
    pub lists_open: bool,
    pub add_open: bool,
    pub filter_open: bool,
    /// Only favourites, on top of the view.
    pub fav_only: bool,
    pub min_rating: Option<u8>,
    /// Picked tags: a group needs all of its tags, and several groups are alternatives.
    pub tag_groups: TagGroups<String>,
    /// What was typed to find a tag in the filter.
    pub tag_query: String,
    /// The delete-list button was pressed once and asks for a second press.
    pub confirm_delete: bool,
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
                || paths::db_path(&faves_schema::META),
                std::path::PathBuf::from,
            );
        let db = Db::open(&path, &faves_schema::SCHEMA).unwrap_or_else(|e| {
            eprintln!("error: cannot open {}: {e}", path.display());
            std::process::exit(1);
        });
        let mut app = Self {
            db,
            view: View::Favorites,
            lists: Vec::new(),
            list_choices: Vec::new(),
            entries: Vec::new(),
            tags: Vec::new(),
            tag_names: Vec::new(),
            sort: Sort::Manual,
            search: Search::default(),
            quick: String::new(),
            details: None,
            settings_open: false,
            compact: false,
            sort_open: false,
            menu_open: false,
            menu_page: 0,
            lists_open: false,
            add_open: false,
            filter_open: false,
            fav_only: false,
            min_rating: None,
            tag_groups: TagGroups::new(),
            tag_query: String::new(),
            confirm_delete: false,
            toast: None,
            desktop_dark: false,
            mode: Mode::Follow,
        };
        app.reload();
        // Start on the first list, or on the favourites when there are none.
        if let Some(first) = app.lists.first() {
            app.view = View::List(first.list.id);
        }
        // Development switches, for screenshots of states that need a click to reach.
        if std::env::var_os("FAVES_GUI_FAVORITES").is_some() {
            app.view = View::Favorites;
        }
        if let Some(id) = std::env::var("FAVES_GUI_LIST")
            .ok()
            .and_then(|v| v.parse().ok())
        {
            app.view = View::List(id);
        }
        if let Ok(q) = std::env::var("FAVES_GUI_SEARCH") {
            app.search.toggle();
            app.search.input(q);
        }
        app.settings_open = std::env::var_os("FAVES_GUI_SETTINGS").is_some();
        app.compact = std::env::var_os("FAVES_GUI_COMPACT").is_some();
        app.sort_open = std::env::var_os("FAVES_GUI_SORT_OPEN").is_some();
        app.menu_open = std::env::var_os("FAVES_GUI_MENU_OPEN").is_some();
        app.menu_page = std::env::var("FAVES_GUI_MENU_PAGE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        app.lists_open = std::env::var_os("FAVES_GUI_LISTS").is_some();
        app.add_open = std::env::var_os("FAVES_GUI_ADD").is_some();
        app.filter_open = std::env::var_os("FAVES_GUI_FILTER").is_some();
        app.fav_only = std::env::var_os("FAVES_GUI_FAV_ONLY").is_some();
        app.min_rating = std::env::var("FAVES_GUI_MIN_RATING")
            .ok()
            .and_then(|v| v.parse().ok());
        if let Ok(t) = std::env::var("FAVES_GUI_TAG") {
            app.tag_groups =
                TagGroups::from_parts(vec![t.split(',').map(String::from).collect()], 0);
        }
        match std::env::var("FAVES_GUI_THEME").as_deref() {
            Ok("light") => app.mode = Mode::Light,
            Ok("dark") => app.mode = Mode::Dark,
            _ => {}
        }
        app.reload();
        if let Some(id) = std::env::var("FAVES_GUI_SELECT")
            .ok()
            .and_then(|v| v.parse().ok())
        {
            app.open(id);
        }
        // Open an entry a moment after start, so the details fade in on screen.
        let later: Option<i64> = std::env::var("FAVES_GUI_SELECT_LATER")
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
                .map(move |()| Message::Open(id)),
            );
        }
        (app, Task::batch(tasks))
    }

    pub fn title(&self) -> String {
        "Faves".into()
    }

    pub fn theme(&self) -> iad::Theme {
        organiser_ui::shell::theme(self.mode.is_dark(self.desktop_dark))
    }

    pub fn subscription(&self) -> Subscription<Message> {
        iad::utils::follow_theme().map(Message::SystemTheme)
    }

    // ---- what the current view holds -----------------------------------------------------

    /// The list being shown, if the view is a list.
    pub fn current(&self) -> Option<&ListSummary> {
        match self.view {
            View::List(id) => self.lists.iter().find(|s| s.list.id == id),
            View::Favorites => None,
        }
    }

    /// The list that holds an entry.
    pub fn list_of(&self, entry: &Entry) -> Option<&List> {
        self.lists
            .iter()
            .map(|s| &s.list)
            .find(|l| l.id == entry.list_id)
    }

    fn filter(&self) -> Filter {
        Filter {
            list: match self.view {
                View::List(id) => Some(id),
                View::Favorites => None,
            },
            text: self.search.query().map(String::from),
            favorites: self.view == View::Favorites || self.fav_only,
            min_rating: self.min_rating,
            tags: self.tag_filter(),
            sort: self.sort,
            ..Filter::default()
        }
    }

    /// The picked tag groups as the engine's filter: the groups are alternatives (one OR block),
    /// and a group needs every one of its tags.
    fn tag_filter(&self) -> TagFilter {
        let leaves: Vec<TagFilter> = self
            .tag_groups
            .groups()
            .iter()
            .filter(|g| !g.is_empty())
            .map(|g| TagFilter {
                all: g.clone(),
                ..TagFilter::default()
            })
            .collect();
        if leaves.is_empty() {
            TagFilter::default()
        } else {
            TagFilter {
                blocks: vec![leaves],
                ..TagFilter::default()
            }
        }
    }

    /// How many filters narrow the entries: favourites, a rating, and each picked tag.
    pub fn filter_count(&self) -> usize {
        usize::from(self.fav_only)
            + usize::from(self.min_rating.is_some())
            + self.tag_groups.groups().iter().map(Vec::len).sum::<usize>()
    }

    fn clear_filters(&mut self) {
        self.fav_only = false;
        self.min_rating = None;
        self.tag_groups = TagGroups::new();
        self.tag_query.clear();
    }

    fn reload(&mut self) {
        match views::lists(&self.db) {
            Ok(l) => {
                self.list_choices = l
                    .iter()
                    .map(|s| ListChoice {
                        id: s.list.id,
                        title: s.list.title.clone(),
                    })
                    .collect();
                self.lists = l;
            }
            Err(e) => self.toast = Some(e.to_string()),
        }
        // A list that no longer exists falls back to the favourites.
        if let View::List(id) = self.view
            && !self.lists.iter().any(|s| s.list.id == id)
        {
            self.view = View::Favorites;
        }
        match views::entries(&self.db, &self.filter()) {
            Ok(e) => self.entries = e,
            Err(e) => self.toast = Some(e.to_string()),
        }
        if let Ok(t) = self.db.tags() {
            self.tag_names = t.iter().map(|t| t.name.clone()).collect();
            self.tags = t;
        }
    }

    fn fail(&mut self, e: impl std::fmt::Display) {
        self.toast = Some(e.to_string());
    }

    // ---- details --------------------------------------------------------------------------

    fn open(&mut self, id: i64) {
        match self.db.get(id) {
            Ok(item) => {
                let entry = Entry::from(&item);
                self.details = Some(Details {
                    title: entry.title.clone(),
                    notes: text_editor::Content::with_text(&entry.notes),
                    entry,
                    adding_tag: false,
                    tag_text: String::new(),
                    error: None,
                });
            }
            Err(e) => self.fail(e),
        }
    }

    /// Re-read the selected entry without touching what is typed.
    fn refresh_details(&mut self) {
        let Some(d) = &self.details else { return };
        match self.db.get(d.entry.id) {
            Ok(item) => {
                if let Some(d) = &mut self.details {
                    d.entry = Entry::from(&item);
                }
            }
            Err(_) => self.details = None,
        }
    }

    /// Save a change to the selected entry, then show what the database now holds.
    fn edit(&mut self, edit: EntryEdit) {
        let Some(d) = &self.details else { return };
        let id = d.entry.id;
        let result = lifecycle::edit_entry(&mut self.db, id, &edit);
        if let Some(d) = &mut self.details {
            d.error = result.err().map(|e| e.to_string());
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
        let mut tags = d.entry.tags.clone();
        f(&mut tags);
        let id = d.entry.id;
        if let Err(e) = lifecycle::set_tags(&mut self.db, id, &tags)
            && let Some(d) = &mut self.details
        {
            d.error = Some(e.to_string());
        }
        self.refresh_details();
        self.reload();
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SystemTheme(s) => self.desktop_dark = s.scheme == Scheme::Dark,
            Message::Window(a) => return iced::window::oldest().and_then(move |id| a.perform(id)),
            Message::Quit | Message::Captured => return iced::exit(),
            Message::Show(v) => {
                self.view = v;
                self.details = None;
                self.lists_open = false;
                self.confirm_delete = false;
                self.reload();
            }
            Message::NewList => {
                match lifecycle::create_list(&mut self.db, "New list", false, false) {
                    Ok(l) => {
                        self.view = View::List(l.id);
                        self.details = None;
                        self.lists_open = false;
                        self.settings_open = true;
                        self.reload();
                    }
                    Err(e) => self.fail(e),
                }
            }
            Message::Compact(on) => self.compact = on,
            Message::OpenLists => self.lists_open = true,
            Message::CloseLists => self.lists_open = false,
            Message::OpenAdd => self.add_open = true,
            Message::CloseAdd => self.add_open = false,
            Message::OpenFilter => self.filter_open = true,
            Message::CloseFilter => self.filter_open = false,
            Message::ClearFilters => {
                self.clear_filters();
                self.reload();
            }
            Message::FavOnly(on) => {
                self.fav_only = on;
                self.reload();
            }
            Message::MinRating(r) => {
                self.min_rating = r;
                self.reload();
            }
            Message::TagGroups(event) => {
                self.tag_groups.apply(event);
                self.reload();
            }
            Message::TagQuery(q) => self.tag_query = q,
            Message::ToggleSearch => {
                self.search.toggle();
                self.reload();
            }
            Message::SearchInput(s) => {
                self.search.input(s);
                self.reload();
            }
            Message::Escape => {
                if self.add_open {
                    self.add_open = false;
                } else if self.filter_open {
                    self.filter_open = false;
                } else if self.lists_open {
                    self.lists_open = false;
                } else if self.settings_open {
                    self.settings_open = false;
                    self.confirm_delete = false;
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
            Message::QuickText(s) => self.quick = s,
            Message::QuickSubmit => {
                let Some(list) = self.current().map(|s| s.list.id) else {
                    return Task::none();
                };
                let title = self.quick.trim().to_string();
                if title.is_empty() {
                    return Task::none();
                }
                match lifecycle::add_entry(&mut self.db, list, &NewEntry::new(&title)) {
                    Ok(e) => {
                        self.quick.clear();
                        self.add_open = false;
                        self.reload();
                        self.open(e.id);
                    }
                    Err(e) => self.fail(e),
                }
            }
            Message::Sort(SortChoice(s)) => {
                self.sort = s;
                self.reload();
            }
            Message::Fave(id) => {
                let now = self
                    .entries
                    .iter()
                    .find(|e| e.id == id)
                    .is_some_and(|e| e.favorite);
                if let Err(e) = lifecycle::set_favorite(&mut self.db, id, !now) {
                    self.fail(e);
                }
                self.refresh_details();
                self.reload();
            }
            Message::Open(id) => self.open(id),
            Message::CloseDetails => self.details = None,
            Message::DTitle(s) => {
                if self.with_details(|d| d.title = s.clone()) && !s.trim().is_empty() {
                    self.edit(EntryEdit {
                        title: Some(s),
                        ..EntryEdit::default()
                    });
                }
            }
            Message::DNotes(a) => {
                let changed = a.is_edit();
                if self.with_details(|d| d.notes.perform(a)) && changed {
                    let text = self
                        .details
                        .as_ref()
                        .map(|d| d.notes.text())
                        .unwrap_or_default();
                    self.edit(EntryEdit {
                        notes: Some(text.trim_end().to_string()),
                        ..EntryEdit::default()
                    });
                }
            }
            Message::DFavorite(on) => self.edit(EntryEdit {
                favorite: Some(on),
                ..EntryEdit::default()
            }),
            Message::DRating(r) => self.edit(EntryEdit {
                rating: Some(r),
                ..EntryEdit::default()
            }),
            Message::DStatus(StatusChoice(s)) => self.edit(EntryEdit {
                status: Some(s),
                ..EntryEdit::default()
            }),
            Message::DMove(choice) => {
                let Some(d) = &self.details else {
                    return Task::none();
                };
                let id = d.entry.id;
                match lifecycle::move_entry(&mut self.db, id, choice.id) {
                    Ok(_) => {
                        // It left this list; follow it, so the card does not vanish from view.
                        if matches!(self.view, View::List(_)) {
                            self.view = View::List(choice.id);
                        }
                        self.refresh_details();
                        self.reload();
                    }
                    Err(e) => self.fail(e),
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
                self.with_details(|d| {
                    d.tag_text.clear();
                    d.adding_tag = !name.is_empty();
                });
                if !name.is_empty() {
                    self.set_tags(|tags| tags.push(name));
                }
            }
            Message::DTagRemove(name) => self.set_tags(|tags| tags.retain(|t| *t != name)),
            Message::DDelete => {
                let Some(d) = &self.details else {
                    return Task::none();
                };
                let id = d.entry.id;
                match self.db.delete(id) {
                    Ok(_) => self.toast = Some("Deleted".into()),
                    Err(e) => self.fail(e),
                }
                self.details = None;
                self.reload();
            }
            Message::OpenSettings => self.settings_open = true,
            Message::CloseSettings => {
                self.settings_open = false;
                self.confirm_delete = false;
            }
            Message::SetListTitle(s) => {
                if let (Some(id), false) = (self.current().map(|c| c.list.id), s.trim().is_empty())
                {
                    let edit = ListEdit {
                        title: Some(s),
                        ..ListEdit::default()
                    };
                    if let Err(e) = lifecycle::edit_list(&mut self.db, id, &edit) {
                        self.fail(e);
                    }
                    self.reload();
                }
            }
            Message::SetRated(on) => self.edit_current_list(ListEdit {
                rated: Some(on),
                ..ListEdit::default()
            }),
            Message::SetTracked(on) => self.edit_current_list(ListEdit {
                tracked: Some(on),
                ..ListEdit::default()
            }),
            Message::DeleteList => {
                let Some(id) = self.current().map(|c| c.list.id) else {
                    return Task::none();
                };
                if !self.confirm_delete {
                    self.confirm_delete = true;
                    return Task::none();
                }
                match lifecycle::delete_list(&mut self.db, id) {
                    Ok(n) => {
                        self.toast = Some(format!(
                            "Deleted the list and {} entries",
                            n.saturating_sub(1)
                        ))
                    }
                    Err(e) => self.fail(e),
                }
                self.settings_open = false;
                self.confirm_delete = false;
                self.details = None;
                self.view = View::Favorites;
                self.reload();
            }
            Message::DismissToast => self.toast = None,
        }
        Task::none()
    }

    fn edit_current_list(&mut self, edit: ListEdit) {
        let Some(id) = self.current().map(|c| c.list.id) else {
            return;
        };
        if let Err(e) = lifecycle::edit_list(&mut self.db, id, &edit) {
            self.fail(e);
        }
        self.refresh_details();
        self.reload();
    }
}
