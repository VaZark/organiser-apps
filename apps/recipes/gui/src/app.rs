//! State, messages and update. Every change goes through `recipes-schema` and the engine; the view
//! never touches the database.

use std::path::PathBuf;

use chrono::{Local, NaiveDate};
use iced::widget::text_editor;
use iced::{Subscription, Task};
use organiser_core::paths;
use organiser_engine::{Db, Filters, Tag};
use organiser_ui::components::tag_query::{count as tag_count, engine_filter};
use organiser_ui::iad::primitives::date::Date as IadDate;
use organiser_ui::iad::primitives::rich_text as rt;
use organiser_ui::iad::primitives::tag_groups::{GroupEvent, TagGroups};
use organiser_ui::iad::{self, Scheme};
use organiser_ui::primitives::scheme::Mode;
use organiser_ui::primitives::search::Search;
use organiser_ui::primitives::text::ellipsize;
use organiser_ui::widgets::preferences::{Changed, PrefMsg, Preferences};
use organiser_ui::widgets::tag_manager::{TagManager, TagMsg};
use recipes_schema::lifecycle::{self, NewRecipe};
use recipes_schema::mealtime::MealTime;
use recipes_schema::model::{Meal, Recipe, RecipeEdit};
use recipes_schema::shopping::Item;
use recipes_schema::views::{self, Filter, Sort};

/// What the content shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Every entry of every list.
    All,
    /// The favourites of every list.
    /// What was cooked, and what is planned, day by day.
    Calendar,
    /// What you have, and what to get.
    Fridge,
}

#[derive(Debug, Clone)]
pub enum Message {
    SystemTheme(iad::utils::ThemeSettings),
    Window(iad::windows::Action),
    Quit,
    Show(View),
    /// Cards, or one compact line per entry.
    Compact(bool),
    /// Open the edit page on a new entry. Nothing is saved until `DCreate`.
    New,
    /// Save the new entry being edited.
    DCreate,
    // The filter, opened from the search.
    OpenFilter,
    CloseFilter,
    ClearFilters,
    /// The filters the schema offers changed.
    Fields(Filters),
    Tags(TagMsg),
    TagGroups(GroupEvent<String>),
    OpenPrefs,
    ClosePrefs,
    Pref(PrefMsg),
    /// Recipes' own setting: cards, or one compact line per entry.
    PrefCompact(bool),
    TagQuery(String),
    ToggleSearch,
    SearchInput(String),
    Escape,
    Sort(SortChoice),
    /// Press an entry's heart on its card.
    Fave(i64),
    // The selected entry.
    Open(i64),
    CloseDetails,
    DTitle(String),
    DNotes(text_editor::Action),
    /// Show the notes drawn (true) or their Markdown source (false).
    DPreview(bool),
    /// Task `n` of the drawn notes was ticked or cleared.
    DTask(usize, bool),
    /// A link in the drawn notes was pressed.
    OpenLink(String),
    DFavorite(bool),
    /// A star was pressed: that rating, or none.
    DRating(Option<u8>),
    DTagStart,
    DTagText(String),
    DTagAdd(String),
    DTagRemove(String),
    DDelete,
    // The settings of the current list.
    DismissToast,
    Captured,
    // The open recipe: read or edit, its ingredients, steps and photos.
    DEdit(bool),
    IngAdd,
    IngName(i64, String),
    IngQty(i64, String),
    IngUnit(i64, String),
    IngOptional(i64, bool),
    IngMove(i64, i32),
    IngDelete(i64),
    StepAdd,
    StepText(i64, text_editor::Action),
    StepMove(i64, i32),
    StepDelete(i64),
    /// Choose pictures for the recipe (`None`) or for one step.
    PickPhotos(Option<i64>),
    PhotosPicked(Option<i64>, Vec<PathBuf>),
    RemovePhoto(Option<i64>, String),
    CookedToday,
    /// Put the open recipe on the calendar: opens the calendar on it.
    PlanIt,
    // The calendar.
    /// Open the dialog that logs a meal, on this day (or today) and perhaps a recipe.
    OpenLog(Option<NaiveDate>, Option<RecipeChoice>),
    /// A place chosen in the narrow header's dropdown.
    Go(Section),
    CloseLog,
    LogDateText(String),
    LogDatePick(IadDate),
    LogDateView(IadDate),
    LogTime(MealChoice),
    LogServings(String),
    LogSave,
    CalRecipe(RecipeChoice),
    MealDelete(i64),
    // The fridge.
    /// Edit one fridge entry (`Some`), or stop editing (`None`).
    FridgeEdit(Option<i64>),
    /// A fridge row was dragged to a new place.
    FridgeList(organiser_ui::iad::primitives::reorder::Event),
    StockExpires(String),
    StockRowExpires(i64, String),
    DServings(String),
    OpenShop,
    CloseShop,
    ShopPick(RecipeChoice),
    ShopAdd,
    ShopPortions(i64, f64),
    ShopRemove(i64),
    ShopPlanned,
    ShopClear,
    ShopCopy,
    ShopSave,
    ShopSaved(Option<PathBuf>),
    StockName(String),
    StockQty(String),
    StockUnit(String),
    StockAdd,
    StockRowName(i64, String),
    StockRowQty(i64, String),
    StockRowUnit(i64, String),
    StockDelete(i64),
}

/// The three places, for the narrow header's dropdown.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Section {
    Recipes,
    Log,
    Fridge,
}

impl Section {
    pub const ALL: &'static [Section] = &[Section::Recipes, Section::Log, Section::Fridge];

    pub fn of(view: View) -> Self {
        match view {
            View::Calendar => Section::Log,
            View::Fridge => Section::Fridge,
            _ => Section::Recipes,
        }
    }

    pub fn view(self) -> View {
        match self {
            Section::Recipes => View::All,
            Section::Log => View::Calendar,
            Section::Fridge => View::Fridge,
        }
    }
}

impl std::fmt::Display for Section {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Section::Recipes => "Recipes",
            Section::Log => "Log",
            Section::Fridge => "Fridge",
        })
    }
}

/// A recipe, for the calendar's picker.
#[derive(Debug, Clone, PartialEq)]
pub struct RecipeChoice {
    pub id: i64,
    pub title: String,
}

impl std::fmt::Display for RecipeChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&ellipsize(&self.title, 40))
    }
}

/// A time of day, or none, for the dropdown.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MealChoice(pub Option<MealTime>);

impl MealChoice {
    pub fn all() -> Vec<MealChoice> {
        std::iter::once(MealChoice(None))
            .chain(MealTime::ALL.into_iter().map(|t| MealChoice(Some(t))))
            .collect()
    }
}

impl std::fmt::Display for MealChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.map_or("Any time", MealTime::label))
    }
}

/// A recipe picked for the shopping list, with the portions wanted.
pub struct ShopRow {
    pub recipe: RecipeChoice,
    pub portions: f64,
    pub servings: Option<f64>,
}

/// One step as edited: its text as typed, drawn for reading, and its photos.
pub struct StepRow {
    pub id: i64,
    pub content: text_editor::Content,
    pub doc: Vec<rt::Block>,
    pub photos: Vec<String>,
}

/// One ingredient as edited: the quantity as typed, so "1." and "" survive while typing.
pub struct IngRow {
    pub id: i64,
    pub name: String,
    pub qty: String,
    pub unit: String,
    pub optional: bool,
}

impl IngRow {
    pub fn blank() -> Self {
        Self {
            id: 0,
            name: String::new(),
            qty: String::new(),
            unit: String::new(),
            optional: false,
        }
    }
}

/// One fridge line as edited.
pub struct StockRow {
    pub id: i64,
    pub name: String,
    pub qty: String,
    pub unit: String,
    /// The date as typed (`YYYY-MM-DD` or empty).
    pub expires: String,
    pub date: Option<NaiveDate>,
}

/// A quantity as typed: empty is none, a comma is a decimal point, anything else that is not a
/// non-negative number is refused (`Err`).
pub fn parse_qty(s: &str) -> Result<Option<f64>, ()> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    match s.replace(',', ".").parse::<f64>() {
        Ok(q) if q.is_finite() && q >= 0.0 => Ok(Some(q)),
        _ => Err(()),
    }
}

/// A date as typed: empty is none; otherwise `YYYY-MM-DD`.
pub fn parse_date(s: &str) -> Result<Option<NaiveDate>, ()> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    s.parse().map(Some).map_err(|_| ())
}

pub fn qty_text(q: Option<f64>) -> String {
    q.map(recipes_schema::units::pretty).unwrap_or_default()
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

/// The selected entry. The text fields hold what was typed; each edit is saved at once.
pub struct Details {
    pub entry: Recipe,
    pub title: String,
    pub notes: text_editor::Content,
    /// Draw the notes instead of editing their source.
    pub preview: bool,
    /// The notes drawn, kept so a redraw does not parse them again.
    pub doc: Vec<rt::Block>,
    pub adding_tag: bool,
    pub tag_text: String,
    pub error: Option<String>,
    /// Reading is the default; editing is a choice (and a new recipe starts in it).
    pub edit: bool,
    pub steps: Vec<StepRow>,
    pub ings: Vec<IngRow>,
    /// When it was cooked or planned, newest first.
    pub history: Vec<Meal>,
    /// The ingredient being typed at the end of the list (id 0).
    pub new_ing: IngRow,
}

impl Details {
    /// Whether this is an entry being written, not yet saved: the edit page doubles as the page
    /// that creates one, and a draft has no id.
    pub fn is_new(&self) -> bool {
        self.entry.id == 0
    }
}

pub struct App {
    pub(crate) db: Db,
    pub view: View,
    pub recipes: Vec<Recipe>,
    pub tags: Vec<Tag>,
    pub tag_names: Vec<String>,
    pub sort: Sort,
    pub search: Search,
    pub details: Option<Details>,
    /// One compact line per entry instead of cards.
    pub compact: bool,
    /// Development only: draw the sort popover open (for screenshots).
    pub sort_open: bool,
    /// Development only: draw the header menu open (for screenshots).
    pub menu_open: bool,
    /// Development only: with the menu open, start on this page (1 is Sort).
    pub menu_page: usize,
    pub filter_open: bool,
    /// Only favourites, on top of the view.
    /// What was chosen in the filters the schema offers (favourite, rating).
    pub filters: Filters,
    pub tag_manager: TagManager,
    /// Picked tags: a group needs all of its tags, and several groups are alternatives.
    pub tag_groups: TagGroups<String>,
    /// The shared settings and the Preferences sheet over them.
    pub prefs: Preferences,
    /// What was typed to find a tag in the filter.
    pub tag_query: String,
    /// The delete-list button was pressed once and asks for a second press.
    pub toast: Option<String>,
    pub desktop_dark: bool,
    pub mode: Mode,
    pub today: NaiveDate,
    /// Every meal, for the log.
    pub meals: Vec<Meal>,
    pub recipe_choices: Vec<RecipeChoice>,
    pub cal_recipe: Option<RecipeChoice>,
    // The dialog that logs a meal.
    pub log_open: bool,
    pub log_text: String,
    pub log_view: IadDate,
    pub log_time: MealChoice,
    pub meal_choices: Vec<MealChoice>,
    pub log_servings: String,
    pub shop_changed: Option<i64>,
    pub stock: Vec<StockRow>,
    pub stock_new: (String, String, String, String),
    /// The fridge entry being edited; the rest are read only.
    pub fridge_edit: Option<i64>,
    /// The fridge entries (by id) in their saved order, always draggable.
    pub stock_order: organiser_ui::iad::primitives::reorder::ReorderState<i64>,
    // The shopping list being prepared.
    pub shop_open: bool,
    pub shop: Vec<ShopRow>,
    pub shop_pick: Option<RecipeChoice>,
    pub shop_items: Vec<Item>,
}

impl App {
    pub fn boot() -> (Self, Task<Message>) {
        let args: Vec<String> = std::env::args().collect();
        let path = args
            .iter()
            .position(|a| a == "--db")
            .and_then(|i| args.get(i + 1))
            .map_or_else(
                || paths::db_path(&recipes_schema::META),
                std::path::PathBuf::from,
            );
        let db = Db::open(&path, &recipes_schema::SCHEMA).unwrap_or_else(|e| {
            eprintln!("error: cannot open {}: {e}", path.display());
            std::process::exit(1);
        });
        let prefs = Preferences::load();
        let mode = Mode::from(prefs.settings.appearance.theme);
        let prefs_compact = prefs.settings.recipes.compact;
        let today = Local::now().date_naive();
        let mut app = Self {
            db,
            view: View::All,
            recipes: Vec::new(),
            tags: Vec::new(),
            tag_names: Vec::new(),
            sort: Sort::Manual,
            search: Search::default(),
            details: None,
            compact: prefs_compact,
            sort_open: false,
            menu_open: false,
            menu_page: 0,
            filter_open: false,
            filters: Filters::default(),
            tag_manager: TagManager::default(),
            tag_groups: TagGroups::new(),
            prefs,
            tag_query: String::new(),
            toast: None,
            desktop_dark: false,
            mode,
            today,
            meals: Vec::new(),
            recipe_choices: Vec::new(),
            cal_recipe: None,
            log_open: false,
            log_text: today.to_string(),
            log_view: IadDate::parse(&today.to_string()).unwrap_or_else(IadDate::today),
            log_time: MealChoice(None),
            meal_choices: MealChoice::all(),
            log_servings: "1".into(),
            shop_changed: None,
            stock: Vec::new(),
            stock_new: Default::default(),
            fridge_edit: None,
            stock_order: Default::default(),
            shop_open: false,
            shop: Vec::new(),
            shop_pick: None,
            shop_items: Vec::new(),
        };
        app.reload();
        // Development switches, for screenshots of states that need a click to reach.
        if std::env::var_os("RECIPES_GUI_FAVORITES").is_some() {
            app.filters.toggle("favorite", serde_json::json!(true));
        }
        if std::env::var_os("RECIPES_GUI_LOG").is_some() {
            app.view = View::Calendar;
            let _ = app.update(Message::OpenLog(None, None));
        }
        if std::env::var_os("RECIPES_GUI_CALENDAR").is_some() {
            app.view = View::Calendar;
        }
        // `<stock id>`: that fridge entry is being edited.
        app.fridge_edit = std::env::var("RECIPES_GUI_FRIDGE_EDIT")
            .ok()
            .and_then(|v| v.parse().ok());
        if std::env::var_os("RECIPES_GUI_FRIDGE").is_some() {
            app.view = View::Fridge;
        }
        // `YYYY-MM-DD`: pretend today is this day (for screenshots of a given month).
        if let Some(d) = std::env::var("RECIPES_GUI_TODAY")
            .ok()
            .and_then(|v| v.parse::<NaiveDate>().ok())
        {
            app.today = d;
            app.log_text = d.to_string();
        }
        if let Ok(q) = std::env::var("RECIPES_GUI_SEARCH") {
            app.search.toggle();
            app.search.input(q);
        }
        if let Ok(ids) = std::env::var("RECIPES_GUI_SHOP") {
            // `3,5`: open the shopping list with these recipes picked.
            for id in ids.split(',').filter_map(|v| v.parse::<i64>().ok()) {
                if let Ok(i) = app.db.get(id) {
                    let r = Recipe::from(&i);
                    app.shop.push(ShopRow {
                        recipe: RecipeChoice { id, title: r.title },
                        portions: r.servings.unwrap_or(1.0) * 2.0,
                        servings: r.servings,
                    });
                }
            }
            app.rebuild_shop();
            app.shop_open = true;
        }
        app.compact |= std::env::var_os("RECIPES_GUI_COMPACT").is_some();
        app.sort_open = std::env::var_os("RECIPES_GUI_SORT_OPEN").is_some();
        app.menu_open = std::env::var_os("RECIPES_GUI_MENU_OPEN").is_some();
        app.menu_page = std::env::var("RECIPES_GUI_MENU_PAGE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        app.filter_open = std::env::var_os("RECIPES_GUI_FILTER").is_some();
        if std::env::var_os("RECIPES_GUI_FAV_ONLY").is_some() {
            app.filters.toggle("favorite", serde_json::json!(true));
        }
        if let Some(n) = std::env::var("RECIPES_GUI_MIN_RATING")
            .ok()
            .and_then(|v| v.parse::<u8>().ok())
        {
            app.filters
                .set_range("rating", Some(serde_json::json!(n)), None);
        }
        if let Ok(t) = std::env::var("RECIPES_GUI_TAG") {
            // `a,b;c` is two groups: a and b together, or c.
            app.tag_groups = TagGroups::from_parts(
                t.split(';')
                    .map(|g| g.split(',').map(String::from).collect())
                    .collect(),
                0,
            );
        }
        if std::env::var_os("RECIPES_GUI_NEW").is_some() {
            let _ = app.update(Message::New);
        }
        if let Ok(page) = std::env::var("RECIPES_GUI_PREFS") {
            app.prefs.open = true;
            // Asking for a page opens it, on a narrow window too.
            app.prefs.page_open = true;
            app.prefs.page = match page.as_str() {
                "tags" => organiser_ui::widgets::preferences::Page::Tags,
                "app" => organiser_ui::widgets::preferences::Page::App,
                _ => organiser_ui::widgets::preferences::Page::Appearance,
            };
        }
        match std::env::var("RECIPES_GUI_THEME").as_deref() {
            Ok("light") => app.mode = Mode::Light,
            Ok("dark") => app.mode = Mode::Dark,
            _ => {}
        }
        app.reload();
        if let Some(id) = std::env::var("RECIPES_GUI_SELECT")
            .ok()
            .and_then(|v| v.parse().ok())
        {
            app.open(id);
            if std::env::var_os("RECIPES_GUI_EDIT").is_some() {
                app.with_details(|d| d.edit = true);
            }
        }
        // Open an entry a moment after start, so the details fade in on screen.
        let later: Option<i64> = std::env::var("RECIPES_GUI_SELECT_LATER")
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
        "Recipes".into()
    }

    pub fn theme(&self) -> iad::Theme {
        organiser_ui::shell::theme(self.mode.is_dark(self.desktop_dark))
    }

    pub fn subscription(&self) -> Subscription<Message> {
        iad::utils::follow_theme().map(Message::SystemTheme)
    }

    // ---- what the current view holds -----------------------------------------------------

    fn filter(&self) -> Filter {
        Filter {
            text: self.search.query().map(String::from),
            fields: self.filters.clone(),
            tags: engine_filter(&self.tag_groups),
            sort: self.sort,
            ..Filter::default()
        }
    }

    /// How many filters narrow the recipes: the schema's own, and each picked tag.
    pub fn filter_count(&self) -> usize {
        self.filters.count() + tag_count(&self.tag_groups)
    }

    fn clear_filters(&mut self) {
        self.filters.clear();
        self.tag_groups = TagGroups::new();
        self.tag_query.clear();
    }

    pub(crate) fn reload(&mut self) {
        match views::recipes(&self.db, &self.filter()) {
            Ok(e) => self.recipes = e,
            Err(e) => self.toast = Some(e.to_string()),
        }
        self.reload_extras();
        // Every tag has a colour of its own, drawn the same everywhere.
        let _ = self.db.ensure_tag_colors(&self.prefs.settings.palette());
        if let Ok(t) = self.db.tags() {
            organiser_ui::components::chip::set_tag_colors(&t);
            self.tag_manager.sync(&t, &self.prefs.settings.palette());
            self.tag_names = t.iter().map(|t| t.name.clone()).collect();
            self.tags = t;
        }
    }

    pub(crate) fn fail(&mut self, e: impl std::fmt::Display) {
        self.toast = Some(e.to_string());
    }

    // ---- details --------------------------------------------------------------------------

    pub(crate) fn open(&mut self, id: i64) {
        match self.db.get(id) {
            Ok(item) => {
                let entry = Recipe::from(&item);
                self.details = Some(Details {
                    title: entry.title.clone(),
                    preview: std::env::var_os("RECIPES_GUI_PREVIEW").is_some(),
                    doc: organiser_ui::components::rich::render(&entry.notes),
                    notes: text_editor::Content::with_text(&entry.notes),
                    entry,
                    adding_tag: false,
                    tag_text: String::new(),
                    error: None,
                    edit: false,
                    steps: Vec::new(),
                    ings: Vec::new(),
                    history: Vec::new(),
                    new_ing: IngRow::blank(),
                });
                self.load_parts();
            }
            Err(e) => self.fail(e),
        }
    }

    /// Read the open recipe's steps, ingredients, history and what is missing from the database.
    pub(crate) fn load_parts(&mut self) {
        let Some(d) = &self.details else { return };
        if d.is_new() {
            return;
        }
        let id = d.entry.id;
        let steps = views::steps(&self.db, id).unwrap_or_default();
        let ings = views::ingredients(&self.db, id).unwrap_or_default();
        let history = views::meals_for(&self.db, id).unwrap_or_default();
        if let Some(d) = &mut self.details {
            d.steps = steps
                .into_iter()
                .map(|s| StepRow {
                    id: s.id,
                    doc: organiser_ui::components::rich::render(&s.text),
                    content: text_editor::Content::with_text(&s.text),
                    photos: s.photos,
                })
                .collect();
            d.ings = ings
                .into_iter()
                .map(|i| IngRow {
                    id: i.id,
                    name: i.name,
                    qty: qty_text(i.quantity),
                    unit: i.unit,
                    optional: i.optional,
                })
                .collect();
            d.history = history;
        }
    }

    /// Only the history, for after the calendar changed.
    pub(crate) fn load_shopping(&mut self) {
        let Some(d) = &self.details else { return };
        if d.is_new() {
            return;
        }
        let history = views::meals_for(&self.db, d.entry.id).unwrap_or_default();
        if let Some(d) = &mut self.details {
            d.history = history;
        }
    }

    /// Re-read the selected entry without touching what is typed.
    pub(crate) fn refresh_details(&mut self) {
        let Some(d) = &self.details else { return };
        if d.is_new() {
            return;
        }
        match self.db.get(d.entry.id) {
            Ok(item) => {
                if let Some(d) = &mut self.details {
                    d.entry = Recipe::from(&item);
                }
            }
            Err(_) => self.details = None,
        }
    }

    /// Save a change to the selected entry, then show what the database now holds.
    pub(crate) fn edit(&mut self, edit: RecipeEdit) {
        let Some(d) = &self.details else { return };
        if d.is_new() {
            // A draft takes the change as typed; it is saved by Create.
            if let Some(d) = &mut self.details {
                let e = &mut d.entry;
                if let Some(t) = edit.title {
                    e.title = t;
                }
                if let Some(n) = edit.notes {
                    e.notes = n;
                }
                if let Some(sv) = edit.servings {
                    e.servings = sv;
                }
                if let Some(f) = edit.favorite {
                    e.favorite = f;
                }
                if let Some(r) = edit.rating {
                    e.rating = r;
                }
            }
            return;
        }
        let id = d.entry.id;
        let result = lifecycle::edit_recipe(&mut self.db, id, &edit);
        if let Some(d) = &mut self.details {
            d.error = result.err().map(|e| e.to_string());
        }
        self.refresh_details();
        self.reload();
    }

    pub(crate) fn with_details(&mut self, f: impl FnOnce(&mut Details)) -> bool {
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
        if d.is_new() {
            self.with_details(|d| d.entry.tags = tags);
            return;
        }
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
                self.fridge_edit = None;
                if v == View::Fridge || v == View::Calendar {
                    self.search.close();
                }
                self.details = None;
                self.reload();
            }
            Message::Go(section) => return self.update(Message::Show(section.view())),
            Message::Compact(on) => self.compact = on,
            Message::OpenFilter => self.filter_open = true,
            Message::CloseFilter => self.filter_open = false,
            Message::ClearFilters => {
                self.clear_filters();
                self.reload();
            }
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
            Message::OpenPrefs => self.prefs.open_first_page(),
            Message::ClosePrefs => self.prefs.open = false,
            Message::Pref(m) => match self.prefs.update(m) {
                Ok(Changed::Theme(t)) => self.mode = Mode::from(t),
                Ok(Changed::Palette | Changed::Nothing) => {}
                Err(e) => self.fail(e),
            },
            Message::PrefCompact(on) => {
                self.prefs.settings.recipes.compact = on;
                if let Err(e) = self.prefs.save() {
                    self.fail(e);
                }
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
                if self.tag_manager.leave_editor() {
                    // Out of a tag's editor first, back to the list.
                } else if self.prefs.leave_page() {
                    // Back from a page to the list of pages (a narrow window).
                } else if self.prefs.open {
                    self.prefs.open = false;
                } else if self.filter_open {
                    self.filter_open = false;
                } else if self.shop_open {
                    self.shop_open = false;
                } else if self.log_open {
                    self.log_open = false;
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
            Message::New if self.view == View::Calendar => {
                return self.more(Message::OpenLog(None, None));
            }
            Message::New => {
                self.details = Some(Details {
                    entry: Recipe {
                        id: 0,
                        title: String::new(),
                        notes: String::new(),
                        photos: Vec::new(),
                        servings: None,
                        favorite: false,
                        rating: None,
                        tags: Vec::new(),
                        position: 0,
                        created: 0,
                        updated: 0,
                    },
                    title: String::new(),
                    notes: text_editor::Content::new(),
                    preview: false,
                    doc: Vec::new(),
                    adding_tag: false,
                    tag_text: String::new(),
                    error: None,
                    // A new recipe is written, not read.
                    edit: true,
                    steps: Vec::new(),
                    ings: Vec::new(),
                    history: Vec::new(),
                    new_ing: IngRow::blank(),
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
                    self.with_details(|d| d.error = Some("Give the recipe a title first.".into()));
                    return Task::none();
                }
                let notes = d.notes.text().trim_end().to_string();
                let new = NewRecipe {
                    title: d.title.trim().to_string(),
                    notes: (!notes.is_empty()).then_some(notes),
                    photos: Vec::new(),
                    servings: d.entry.servings,
                    favorite: d.entry.favorite,
                    // A list only keeps the extras it offers.
                    rating: d.entry.rating,
                    tags: d.entry.tags.clone(),
                };
                match lifecycle::add_recipe(&mut self.db, &new) {
                    Ok(e) => {
                        self.reload();
                        self.open(e.id);
                        // Ingredients, steps and photos come next.
                        self.with_details(|d| d.edit = true);
                    }
                    Err(e) => {
                        self.with_details(|d| d.error = Some(e.to_string()));
                    }
                }
            }
            Message::DPreview(on) => {
                self.with_details(|d| d.preview = on);
            }
            Message::DTask(n, on) => {
                let source = self.details.as_ref().map(|d| d.notes.text());
                if let Some(new) = source.and_then(|s| organiser_markdown::tasks::set(&s, n, on)) {
                    self.with_details(|d| d.notes = text_editor::Content::with_text(&new));
                    self.edit(RecipeEdit {
                        notes: Some(new.trim_end().to_string()),
                        ..RecipeEdit::default()
                    });
                    self.with_details(|d| {
                        d.doc = organiser_ui::components::rich::render(&d.notes.text());
                    });
                }
            }
            Message::OpenLink(url) => {
                if let Err(e) = organiser_ui::shell::open_link(&url) {
                    self.toast = Some(e);
                }
            }
            Message::Sort(SortChoice(s)) => {
                self.sort = s;
                self.reload();
            }
            Message::Fave(id) => {
                let now = self
                    .recipes
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
                    self.edit(RecipeEdit {
                        title: Some(s),
                        ..RecipeEdit::default()
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
                    self.edit(RecipeEdit {
                        notes: Some(text.trim_end().to_string()),
                        ..RecipeEdit::default()
                    });
                    self.with_details(|d| {
                        d.doc = organiser_ui::components::rich::render(&d.notes.text());
                    });
                }
            }
            Message::DFavorite(on) => self.edit(RecipeEdit {
                favorite: Some(on),
                ..RecipeEdit::default()
            }),
            Message::DRating(r) => self.edit(RecipeEdit {
                rating: Some(r),
                ..RecipeEdit::default()
            }),
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
                if d.is_new() {
                    return Task::none();
                }
                let id = d.entry.id;
                match self.db.delete(id) {
                    Ok(_) => {
                        let _ =
                            recipes_schema::photos::gc(&self.db, &recipes_schema::photos::dir());
                        self.toast = Some("Deleted".into());
                    }
                    Err(e) => self.fail(e),
                }
                self.details = None;
                self.reload();
            }
            Message::DismissToast => self.toast = None,
            other => return self.more(other),
        }
        Task::none()
    }
}
