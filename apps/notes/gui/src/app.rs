//! State, messages and update. Every change goes through `notes-schema` and the engine; the view
//! never touches the database.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use iced::widget::text_editor;
use iced::{Color as IcedColor, Subscription, Task};
use notes_schema::color::Color;
use notes_schema::folder::{Folder, FolderEdit, FolderSummary};
use notes_schema::lifecycle::{self, NewNote};
use notes_schema::md_tasks;
use notes_schema::note::{Edit, Note};
use notes_schema::section::{SectionEdit, SectionSummary};
use notes_schema::views::{self, Filter, Scope, Sort};
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
    /// Hide or show the docked sidebar of a wide window.
    ToggleSidebar,
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
    /// Notes' own setting: cards, or a list.
    PrefListView(bool),
    /// Cards, or a list.
    ListView(bool),
    /// Notes' own settings: group by folder in the views of every note; by section on a folder's
    /// page.
    PrefGroupByFolder(bool),
    PrefGroupBySection(bool),
    /// Group by folder in the views of every note.
    GroupByFolder(bool),
    /// Group by section on a folder's page.
    GroupBySection(bool),
    SetSort(Sort),
    // Folders.
    NewFolder,
    OpenFolderSettings(i64),
    CloseFolderSettings,
    SetFolderTitle(String),
    SetFolderRated(bool),
    /// What the folder is for.
    SetFolderNotes(String),
    /// The folder's icon, by name; none is the folder.
    SetFolderIcon(Option<&'static str>),
    /// The folder's colour, picked or typed.
    SetFolderColor(Hsva),
    SetFolderColorText(String),
    /// Delete the folder being set up: asks what becomes of its notes when it has some.
    FolderDeleteAsk,
    FolderDeleteKeep,
    FolderDeleteAll,
    FolderDeleteCancel,
    /// The heart of a card or row: favourite it, or not.
    Fave(i64),
    DFavorite(bool),
    /// A star was pressed: that rating, or none.
    DRating(Option<u8>),
    /// Filed in this folder, or none.
    DFolder(FolderChoice),
    /// In this section of its folder, or none.
    DSection(SectionChoice),
    // Sections of a folder.
    NewSection(i64),
    OpenSectionSettings(i64),
    CloseSectionSettings,
    SetSectionTitle(String),
    SetSectionNotes(String),
    SetSectionColor(Hsva),
    SetSectionColorText(String),
    DeleteSection,
    /// Fold a section of the folder's page, or open it again.
    SectionFold(i64),
    /// A new note in this section of the folder being viewed.
    NewInSection(i64),
    /// A note was picked up.
    DragStart(i64),
    /// The pointer button went up: whatever was being dragged is let go.
    DragEnd,
    /// The note being dragged was dropped on this section of its folder, or (`None`) on none.
    DropInSection(Option<i64>),
    /// A section was picked up by its name, to put it in another place.
    SectionDragStart(i64),
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

/// A folder, or none, for the "file in" dropdown.
#[derive(Debug, Clone, PartialEq)]
pub struct FolderChoice {
    pub id: Option<i64>,
    pub title: String,
}

impl std::fmt::Display for FolderChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&organiser_ui::primitives::text::ellipsize(&self.title, 30))
    }
}

/// A section of the open note's folder, or none, for the "in section" dropdown.
#[derive(Debug, Clone, PartialEq)]
pub struct SectionChoice {
    pub id: Option<i64>,
    pub title: String,
}

impl std::fmt::Display for SectionChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&organiser_ui::primitives::text::ellipsize(&self.title, 30))
    }
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
    /// The sidebar of a wide window is hidden.
    pub sidebar_hidden: bool,
    /// A list of the notes instead of cards.
    pub list_view: bool,
    /// Group by folder in the views of every note.
    pub group_by_folder: bool,
    /// Group by section on a folder's page.
    pub group_by_section: bool,
    pub sort: Sort,
    pub folders: Vec<FolderSummary>,
    pub folder_choices: Vec<FolderChoice>,
    /// The sections of each folder, by folder id.
    pub folder_sections: HashMap<i64, Vec<SectionSummary>>,
    /// The sections of the open note's folder, to choose from.
    pub section_choices: Vec<SectionChoice>,
    /// Sections folded shut on a folder's page.
    pub folded_sections: std::collections::HashSet<i64>,
    /// The section whose settings are open, and what is typed in its name, description and colour.
    pub section_settings: Option<i64>,
    pub section_name: Option<String>,
    pub section_notes: Option<String>,
    pub section_color_text: Option<String>,
    /// The note being dragged between sections.
    pub dragging: Option<i64>,
    /// The section being dragged to another place.
    pub dragging_section: Option<i64>,
    /// The folder whose settings are open.
    pub folder_settings: Option<i64>,
    /// The folder being deleted, waiting for the answer to what becomes of its notes.
    pub confirm_folder_delete: Option<i64>,
    /// What is typed in the folder's name and colour, while it is not a valid one yet.
    pub folder_name: Option<String>,
    pub folder_notes: Option<String>,
    pub folder_color_text: Option<String>,
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

/// `#rrggbb` of an iced colour.
fn paint_hex(c: IcedColor) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        (c.r * 255.0).round() as u8,
        (c.g * 255.0).round() as u8,
        (c.b * 255.0).round() as u8
    )
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
        // The default folder, "Unassigned", exists and holds the notes that are in no other.
        if let Err(e) = lifecycle::ensure_default_folder(&mut db) {
            eprintln!("error: cannot set up the default folder: {e}");
            std::process::exit(1);
        }
        let prefs = Preferences::load();
        let list_view = prefs.settings.notes.list_view;
        let group_by_folder = prefs.settings.notes.group_by_folder;
        let group_by_section = prefs.settings.notes.group_by_section;
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
            sidebar_hidden: false,
            list_view: list_view || std::env::var_os("NOTES_GUI_LIST").is_some(),
            group_by_folder,
            group_by_section,
            sort: Sort::default(),
            folders: Vec::new(),
            folder_choices: Vec::new(),
            folder_sections: HashMap::new(),
            section_choices: Vec::new(),
            folded_sections: Default::default(),
            section_settings: None,
            section_name: None,
            section_notes: None,
            section_color_text: None,
            dragging: None,
            dragging_section: None,
            folder_settings: None,
            confirm_folder_delete: None,
            folder_name: None,
            folder_notes: None,
            folder_color_text: None,
            details: None,
            toast: None,
            desktop_dark: false,
            mode,
        };
        // Development switches, for screenshots of states that need a click to reach.
        match std::env::var("NOTES_GUI_SCOPE").as_deref() {
            Ok("archive") => app.scope = Scope::Archive,
            Ok("trash") => app.scope = Scope::Trash,
            Ok("favorites") => app.scope = Scope::Favorites,
            _ => {}
        }
        if let Some(id) = std::env::var("NOTES_GUI_FOLDER").ok().and_then(|v| v.parse().ok()) {
            app.scope = Scope::Folder(id);
            if std::env::var_os("NOTES_GUI_FOLDER_SETTINGS").is_some() {
                app.folder_settings = Some(id);
            }
        }
        if std::env::var_os("NOTES_GUI_UNGROUPED").is_some() {
            app.group_by_folder = false;
            app.group_by_section = false;
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
        // Settings opened at start show the folder's colour in the picker, as opening them does.
        if let Some(id) = app.folder_settings {
            app.folder_color_text = app
                .folders
                .iter()
                .find(|s| s.folder.id == id)
                .map(|s| paint_hex(app.folder_color(&s.folder)));
        }
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
        Subscription::batch([
            iad::utils::follow_theme().map(Message::SystemTheme),
            // Letting go anywhere ends a drag. The section it landed on has already been told.
            iced::event::listen_with(|event, _, _| match event {
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )) => Some(Message::DragEnd),
                _ => None,
            }),
        ])
    }

    fn filter(&self) -> Filter {
        Filter {
            scope: self.scope,
            text: self.search.query().map(String::from),
            tags: engine_filter(&self.tag_groups),
            fields: self.filters.clone(),
            // A search looks in the archive too, so an old note is still found.
            include_archived: self.search.query().is_some(),
            sort: self.sort,
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
        match views::folders(&self.db) {
            Ok(f) => {
                // A note is always in a folder: the default one, "Unassigned", when no other.
                self.folder_choices = f
                    .iter()
                    .map(|s| FolderChoice {
                        id: Some(s.folder.id),
                        title: s.folder.title.clone(),
                    })
                    .collect();
                self.folders = f;
            }
            Err(e) => self.toast = Some(e.to_string()),
        }
        self.folder_sections = self
            .folders
            .iter()
            .filter_map(|s| {
                views::sections(&self.db, s.folder.id)
                    .ok()
                    .map(|v| (s.folder.id, v))
            })
            .collect();
        self.refresh_section_choices();
        // A folder that no longer exists falls back to all the notes.
        if let Scope::Folder(id) = self.scope
            && !self.folders.iter().any(|s| s.folder.id == id)
        {
            self.scope = Scope::Notes;
            self.reload();
            return;
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

    /// A folder's colour: its own, or one made from its name, so folders are told apart at a
    /// glance.
    pub fn folder_color(&self, folder: &Folder) -> IcedColor {
        folder.color.map(paint).unwrap_or_else(|| {
            if folder.is_default {
                // The default folder is plain unless it has been given a colour.
                organiser_ui::components::color_pick::UNSET
            } else {
                organiser_ui::components::chip::color_of(&folder.title)
            }
        })
    }

    /// The default folder, "Unassigned".
    pub fn default_folder_id(&self) -> Option<i64> {
        self.folders.iter().find(|s| s.folder.is_default).map(|s| s.folder.id)
    }

    /// The section a note is in.
    pub fn section_of(&self, note: &Note) -> Option<&notes_schema::section::Section> {
        let id = note.section?;
        self.folder_sections.get(&note.folder_id?)?.iter().map(|s| &s.section).find(|s| s.id == id)
    }

    /// The folder a note is filed in.
    pub fn folder_of(&self, note: &Note) -> Option<&Folder> {
        let id = note.folder_id?;
        self.folders.iter().map(|s| &s.folder).find(|f| f.id == id)
    }

    /// A note's colour: its own, else its folder's, else none (the default card).
    pub fn note_tint(&self, note: &Note) -> Option<IcedColor> {
        note.color
            .map(paint)
            .or_else(|| self.folder_of(note).map(|f| self.folder_color(f)))
    }

    /// The folder whose settings are open.
    pub fn settings_folder(&self) -> Option<&FolderSummary> {
        let id = self.folder_settings?;
        self.folders.iter().find(|s| s.folder.id == id)
    }

    /// The pinned notes, then the rest. Pinning only groups the live notes.
    pub fn pinned(&self) -> (Vec<&Note>, Vec<&Note>) {
        if matches!(self.scope, Scope::Notes | Scope::Favorites | Scope::Folder(_)) {
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
        self.refresh_section_choices();
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
        self.refresh_section_choices();
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

    /// The sections of the open note's folder, with "No section" first.
    fn refresh_section_choices(&mut self) {
        let folder = self.details.as_ref().and_then(|d| d.note.folder_id);
        self.section_choices = match folder.and_then(|f| self.folder_sections.get(&f)) {
            Some(v) if !v.is_empty() => std::iter::once(SectionChoice {
                id: None,
                title: "No section".into(),
            })
            .chain(v.iter().map(|s| SectionChoice {
                id: Some(s.section.id),
                title: s.section.title.clone(),
            }))
            .collect(),
            _ => Vec::new(),
        };
    }

    fn edit_section(&mut self, id: i64, edit: SectionEdit) {
        if let Err(e) = lifecycle::edit_section(&mut self.db, id, &edit) {
            self.fail(e);
        }
        self.reload();
    }

    fn edit_folder(&mut self, id: i64, edit: FolderEdit) {
        if let Err(e) = lifecycle::edit_folder(&mut self.db, id, &edit) {
            self.fail(e);
        }
        self.refresh_details();
        self.reload();
    }

    /// Delete a folder, keeping its notes (filed in none) or deleting them with it.
    fn delete_folder(&mut self, id: i64, with_notes: bool) {
        let result = if with_notes {
            lifecycle::delete_folder_and_notes(&mut self.db, id)
                .map(|n| format!("Deleted the folder and {n} note{}", if n == 1 { "" } else { "s" }))
        } else {
            lifecycle::delete_folder(&mut self.db, id).map(|()| "Deleted the folder; its notes stay".into())
        };
        match result {
            Ok(m) => self.toast = Some(m),
            Err(e) => self.fail(e),
        }
        self.folder_settings = None;
        self.details = None;
        self.reload();
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
            Message::ToggleSidebar => self.sidebar_hidden = !self.sidebar_hidden,
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
            Message::ListView(on) => self.list_view = on,
            Message::GroupByFolder(on) => self.group_by_folder = on,
            Message::GroupBySection(on) => self.group_by_section = on,
            Message::PrefGroupBySection(on) => {
                self.prefs.settings.notes.group_by_section = on;
                if let Err(e) = self.prefs.save() {
                    self.fail(e);
                }
            }
            Message::PrefGroupByFolder(on) => {
                self.prefs.settings.notes.group_by_folder = on;
                if let Err(e) = self.prefs.save() {
                    self.fail(e);
                }
            }
            Message::SetSort(sort) => {
                self.sort = sort;
                self.reload();
            }
            Message::NewFolder => match lifecycle::create_folder(&mut self.db, "New folder", false) {
                Ok(f) => {
                    self.scope = Scope::Folder(f.id);
                    self.drawer_open = false;
                    self.details = None;
                    self.reload();
                    // Name it at once.
                    self.folder_settings = Some(f.id);
                    self.folder_name = None;
                    self.folder_notes = None;
                    self.folder_color_text = self
                        .folders
                        .iter()
                        .find(|s| s.folder.id == f.id)
                        .map(|s| paint_hex(self.folder_color(&s.folder)));
                }
                Err(e) => self.fail(e),
            },
            Message::OpenFolderSettings(id) => {
                self.folder_settings = Some(id);
                self.folder_name = None;
                self.folder_notes = None;
                self.folder_color_text = self
                    .folders
                    .iter()
                    .find(|s| s.folder.id == id)
                    .map(|s| paint_hex(self.folder_color(&s.folder)));
            }
            Message::CloseFolderSettings => {
                self.folder_settings = None;
                self.folder_name = None;
            }
            Message::SetFolderTitle(t) => {
                if let Some(id) = self.folder_settings {
                    // An empty name waits for more typing; the old one stays until then.
                    if !t.trim().is_empty() {
                        self.edit_folder(id, FolderEdit {
                            title: Some(t.trim().to_string()),
                            ..FolderEdit::default()
                        });
                    }
                    self.folder_name = Some(t);
                }
            }
            Message::SetFolderNotes(t) => {
                if let Some(id) = self.folder_settings {
                    self.edit_folder(id, FolderEdit {
                        notes: Some(t.clone()),
                        ..FolderEdit::default()
                    });
                    self.folder_notes = Some(t);
                }
            }
            Message::SetFolderRated(on) => {
                if let Some(id) = self.folder_settings {
                    self.edit_folder(id, FolderEdit {
                        rated: Some(on),
                        ..FolderEdit::default()
                    });
                }
            }
            Message::SetFolderIcon(name) => {
                if let Some(id) = self.folder_settings {
                    self.edit_folder(id, FolderEdit {
                        icon: Some(name.map(String::from)),
                        ..FolderEdit::default()
                    });
                }
            }
            Message::SetFolderColor(h) => {
                if let Some(id) = self.folder_settings {
                    let [r, g, b] = h.to_rgb8();
                    self.folder_color_text = Some(Color::rgb(r, g, b).hex());
                    self.edit_folder(id, FolderEdit {
                        color: Some(Some(Color::rgb(r, g, b))),
                        ..FolderEdit::default()
                    });
                }
            }
            Message::SetFolderColorText(text) => {
                if let Some(id) = self.folder_settings {
                    let parsed = Hsva::parse(&text).map(|h| {
                        let [r, g, b] = h.to_rgb8();
                        Color::rgb(r, g, b)
                    });
                    self.folder_color_text = Some(text);
                    if let Some(c) = parsed {
                        self.edit_folder(id, FolderEdit {
                            color: Some(Some(c)),
                            ..FolderEdit::default()
                        });
                    }
                }
            }
            Message::FolderDeleteAsk => {
                if let Some(id) = self.folder_settings {
                    // A folder with notes asks what becomes of them first.
                    if self.folders.iter().any(|s| s.folder.id == id && s.notes > 0) {
                        self.confirm_folder_delete = Some(id);
                    } else {
                        self.delete_folder(id, false);
                    }
                }
            }
            Message::FolderDeleteCancel => self.confirm_folder_delete = None,
            Message::FolderDeleteKeep | Message::FolderDeleteAll => {
                let all = matches!(message, Message::FolderDeleteAll);
                if let Some(id) = self.confirm_folder_delete.take() {
                    self.delete_folder(id, all);
                }
            }
            Message::Fave(id) => {
                let now_fave = self.notes.iter().find(|n| n.id == id).is_some_and(|n| n.favorite);
                if let Err(e) = lifecycle::set_favorite(&mut self.db, id, !now_fave) {
                    self.fail(e);
                }
                self.refresh_details();
                self.reload();
            }
            Message::DFavorite(on) => {
                if self.details.as_ref().is_some_and(Details::is_new) {
                    self.with_details(|d| d.note.favorite = on);
                } else {
                    self.act(|db, id| lifecycle::set_favorite(db, id, on).map(|_| ()));
                }
            }
            Message::DRating(r) => {
                if self.details.as_ref().is_some_and(Details::is_new) {
                    self.with_details(|d| d.note.rating = r);
                } else {
                    self.act(|db, id| {
                        lifecycle::edit(
                            db,
                            id,
                            &Edit {
                                rating: Some(r),
                                ..Edit::default()
                            },
                        )
                        .map(|_| ())
                    });
                }
            }
            Message::DSection(choice) => {
                if self.details.as_ref().is_some_and(Details::is_new) {
                    self.with_details(|d| d.note.section = choice.id);
                } else {
                    self.act(|db, id| lifecycle::set_section(db, id, choice.id).map(|_| ()));
                }
            }
            Message::NewSection(folder) => {
                match lifecycle::create_section(&mut self.db, folder, "New section") {
                    Ok(s) => {
                        self.reload();
                        // Name it at once.
                        let _ = self.update(Message::OpenSectionSettings(s.id));
                    }
                    Err(e) => self.fail(e),
                }
            }
            Message::OpenSectionSettings(id) => {
                self.section_settings = Some(id);
                self.section_name = None;
                self.section_notes = None;
                self.section_color_text = self
                    .folder_sections
                    .values()
                    .flatten()
                    .find(|s| s.section.id == id)
                    .and_then(|s| s.section.color.map(|c| c.hex()));
            }
            Message::CloseSectionSettings => self.section_settings = None,
            Message::SetSectionTitle(t) => {
                if let Some(id) = self.section_settings {
                    if !t.trim().is_empty() {
                        self.edit_section(id, SectionEdit {
                            title: Some(t.trim().to_string()),
                            ..SectionEdit::default()
                        });
                    }
                    self.section_name = Some(t);
                }
            }
            Message::SetSectionNotes(t) => {
                if let Some(id) = self.section_settings {
                    self.edit_section(id, SectionEdit {
                        notes: Some(t.clone()),
                        ..SectionEdit::default()
                    });
                    self.section_notes = Some(t);
                }
            }
            Message::SetSectionColor(h) => {
                if let Some(id) = self.section_settings {
                    let [r, g, b] = h.to_rgb8();
                    let c = Color::rgb(r, g, b);
                    self.section_color_text = Some(c.hex());
                    self.edit_section(id, SectionEdit {
                        color: Some(Some(c)),
                        ..SectionEdit::default()
                    });
                }
            }
            Message::SetSectionColorText(text) => {
                if let Some(id) = self.section_settings {
                    let parsed = Hsva::parse(&text).map(|h| {
                        let [r, g, b] = h.to_rgb8();
                        Color::rgb(r, g, b)
                    });
                    self.section_color_text = Some(text);
                    if let Some(c) = parsed {
                        self.edit_section(id, SectionEdit {
                            color: Some(Some(c)),
                            ..SectionEdit::default()
                        });
                    }
                }
            }
            Message::DeleteSection => {
                if let Some(id) = self.section_settings.take() {
                    match lifecycle::delete_section(&mut self.db, id) {
                        Ok(()) => self.toast = Some("Deleted the section; its notes stay".into()),
                        Err(e) => self.fail(e),
                    }
                    self.refresh_details();
                    self.reload();
                }
            }
            Message::SectionFold(id) => {
                if !self.folded_sections.remove(&id) {
                    self.folded_sections.insert(id);
                }
            }
            Message::NewInSection(section) => {
                let _ = self.update(Message::New);
                if let Some(d) = &mut self.details {
                    d.note.section = Some(section);
                }
            }
            Message::DragStart(id) => self.dragging = Some(id),
            Message::DragEnd => {
                self.dragging = None;
                self.dragging_section = None;
            }
            Message::SectionDragStart(id) => self.dragging_section = Some(id),
            Message::DropInSection(section) => {
                // A section dropped on another takes its place.
                if let Some(moved) = self.dragging_section.take() {
                    if let Some(target) = section {
                        let place = self
                            .folder_sections
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
                    if let Err(e) = lifecycle::set_section(&mut self.db, id, section) {
                        self.fail(e);
                    }
                    self.refresh_details();
                    self.reload();
                }
            }
            Message::DFolder(choice) => {
                if self.details.as_ref().is_some_and(Details::is_new) {
                    self.with_details(|d| {
                        if d.note.folder_id != choice.id {
                            d.note.section = None;
                        }
                        d.note.folder_id = choice.id;
                    });
                    self.refresh_section_choices();
                } else {
                    self.act(|db, id| lifecycle::move_note(db, id, choice.id).map(|_| ()));
                    // It left this folder's page; follow it, so it does not vanish from view.
                    if matches!(self.scope, Scope::Folder(_)) {
                        self.scope = choice.id.map_or(Scope::Notes, Scope::Folder);
                        self.reload();
                    }
                }
            }
            Message::PrefListView(on) => {
                self.prefs.settings.notes.list_view = on;
                if let Err(e) = self.prefs.save() {
                    self.fail(e);
                }
            }
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
                } else if self.confirm_folder_delete.is_some() {
                    self.confirm_folder_delete = None;
                } else if self.section_settings.is_some() {
                    self.section_settings = None;
                } else if self.folder_settings.is_some() {
                    self.folder_settings = None;
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
                        // In a folder's page, a new note goes in that folder.
                        // In a folder's page, a new note goes in that folder; elsewhere, in the
                        // default one.
                        folder_id: match self.scope {
                            Scope::Folder(id) => Some(id),
                            _ => self.default_folder_id(),
                        },
                        section: None,
                        title: String::new(),
                        body: String::new(),
                        color: None,
                        pinned: false,
                        favorite: matches!(self.scope, Scope::Favorites),
                        rating: None,
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
                // A rating is kept only where the folder offers them.
                let rated = d
                    .note
                    .folder_id
                    .is_some_and(|id| self.folders.iter().any(|s| s.folder.id == id && s.folder.rated));
                let new = NewNote {
                    title: d.title.trim().to_string(),
                    body: d.body.text().trim_end().to_string(),
                    color: d.note.color,
                    tags: d.note.tags.clone(),
                    folder: d.note.folder_id,
                    section: d.note.section,
                    favorite: d.note.favorite,
                    rating: d.note.rating.filter(|_| rated),
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
                        // A new note belongs in the notes, whatever the page was showing; in a
                        // folder's own page it stays there.
                        if !matches!(self.scope, Scope::Folder(_) | Scope::Favorites) {
                            self.scope = Scope::Notes;
                        }
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
