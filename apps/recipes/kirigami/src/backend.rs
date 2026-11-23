//! The QML-facing object: a thin shell over [`Core`]. Every invokable runs a core action, then
//! re-publishes the lists; a failure becomes the `toast` text.

use core::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use recipes_schema::views::Sort;

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
        #[qproperty(QString, recipes)]
        #[qproperty(QString, tags)]
        #[qproperty(QString, choices)]
        #[qproperty(QString, stock)]
        #[qproperty(QString, calendar)]
        #[qproperty(QString, shop)]
        #[qproperty(QString, toast)]
        #[qproperty(QString, search)]
        #[qproperty(QString, sort)]
        #[qproperty(bool, favorites_only, cxx_name = "favoritesOnly")]
        #[qproperty(i32, min_rating, cxx_name = "minRating")]
        #[qproperty(QString, tag_filter, cxx_name = "tagFilter")]
        #[qproperty(i32, filter_count, cxx_name = "filterCount")]
        #[qproperty(bool, compact)]
        #[qproperty(QString, today)]
        /// Counts every re-publish, so a page showing one recipe knows to re-read it.
        #[qproperty(i32, revision)]
        type Backend = super::BackendRust;

        #[qinvokable]
        fn init(self: Pin<&mut Backend>);
        #[qinvokable]
        fn reload(self: Pin<&mut Backend>);
        /// Re-read after `search`, `sort`, `favoritesOnly`, `minRating` or `tagFilter` changed.
        #[qinvokable]
        #[cxx_name = "applyFilters"]
        fn apply_filters(self: Pin<&mut Backend>);
        #[qinvokable]
        #[cxx_name = "clearFilters"]
        fn clear_filters(self: Pin<&mut Backend>);
        /// Keep the list style for the next start.
        #[qinvokable]
        #[cxx_name = "savePreferences"]
        fn save_preferences(self: Pin<&mut Backend>);
        #[qinvokable]
        fn recipe(self: &Backend, id: i64) -> QString;
        #[qinvokable]
        #[cxx_name = "mealTimes"]
        fn meal_times(self: &Backend) -> QString;
        /// The value after `--name` on the command line, or empty (development flags).
        #[qinvokable]
        fn arg(self: &Backend, name: &QString) -> QString;

        #[qinvokable]
        #[cxx_name = "createRecipe"]
        fn create_recipe(self: Pin<&mut Backend>, title: &QString) -> i64;
        #[qinvokable]
        #[cxx_name = "deleteRecipe"]
        fn delete_recipe(self: Pin<&mut Backend>, id: i64);
        #[qinvokable]
        #[cxx_name = "editTitle"]
        fn edit_title(self: Pin<&mut Backend>, id: i64, title: &QString);
        #[qinvokable]
        #[cxx_name = "editNotes"]
        fn edit_notes(self: Pin<&mut Backend>, id: i64, notes: &QString);
        #[qinvokable]
        #[cxx_name = "editServings"]
        fn edit_servings(self: Pin<&mut Backend>, id: i64, servings: &QString);
        #[qinvokable]
        #[cxx_name = "setFavorite"]
        fn set_favorite(self: Pin<&mut Backend>, id: i64, on: bool);
        #[qinvokable]
        #[cxx_name = "setRating"]
        fn set_rating(self: Pin<&mut Backend>, id: i64, stars: i32);
        /// `tags` is a comma-separated list.
        #[qinvokable]
        #[cxx_name = "setRecipeTags"]
        fn set_recipe_tags(self: Pin<&mut Backend>, id: i64, tags: &QString);

        #[qinvokable]
        #[cxx_name = "addIngredient"]
        fn add_ingredient(
            self: Pin<&mut Backend>,
            recipe: i64,
            name: &QString,
            qty: &QString,
            unit: &QString,
            optional: bool,
        );
        #[qinvokable]
        #[cxx_name = "editIngredient"]
        fn edit_ingredient(
            self: Pin<&mut Backend>,
            id: i64,
            name: &QString,
            qty: &QString,
            unit: &QString,
            optional: bool,
        );
        #[qinvokable]
        #[cxx_name = "moveIngredient"]
        fn move_ingredient(self: Pin<&mut Backend>, id: i64, by: i32);
        #[qinvokable]
        #[cxx_name = "deleteIngredient"]
        fn delete_ingredient(self: Pin<&mut Backend>, id: i64);
        #[qinvokable]
        #[cxx_name = "addStep"]
        fn add_step(self: Pin<&mut Backend>, recipe: i64);
        #[qinvokable]
        #[cxx_name = "editStep"]
        fn edit_step(self: Pin<&mut Backend>, id: i64, text: &QString);
        #[qinvokable]
        #[cxx_name = "moveStep"]
        fn move_step(self: Pin<&mut Backend>, id: i64, by: i32);
        #[qinvokable]
        #[cxx_name = "deleteStep"]
        fn delete_step(self: Pin<&mut Backend>, id: i64);

        /// `step` is a step's id, or -1 for the recipe's own photos; `files` are URLs separated
        /// by newlines.
        #[qinvokable]
        #[cxx_name = "addPhotos"]
        fn add_photos(self: Pin<&mut Backend>, recipe: i64, step: i64, files: &QString);
        #[qinvokable]
        #[cxx_name = "removePhoto"]
        fn remove_photo(self: Pin<&mut Backend>, recipe: i64, step: i64, name: &QString);

        #[qinvokable]
        #[cxx_name = "cookedToday"]
        fn cooked_today(self: Pin<&mut Backend>, recipe: i64);
        #[qinvokable]
        #[cxx_name = "logMeal"]
        fn log_meal(
            self: Pin<&mut Backend>,
            recipe: i64,
            date: &QString,
            time: &QString,
            servings: &QString,
        ) -> bool;
        #[qinvokable]
        #[cxx_name = "deleteMeal"]
        fn delete_meal(self: Pin<&mut Backend>, id: i64);
        #[qinvokable]
        #[cxx_name = "shiftMonth"]
        fn shift_month(self: Pin<&mut Backend>, delta: i32);
        #[qinvokable]
        #[cxx_name = "thisMonth"]
        fn this_month(self: Pin<&mut Backend>);

        #[qinvokable]
        #[cxx_name = "addStock"]
        fn add_stock(
            self: Pin<&mut Backend>,
            name: &QString,
            qty: &QString,
            unit: &QString,
            expires: &QString,
        ) -> bool;
        #[qinvokable]
        #[cxx_name = "editStock"]
        fn edit_stock(
            self: Pin<&mut Backend>,
            id: i64,
            name: &QString,
            qty: &QString,
            unit: &QString,
            expires: &QString,
        ) -> bool;
        #[qinvokable]
        #[cxx_name = "moveStock"]
        fn move_stock(self: Pin<&mut Backend>, id: i64, by: i32);
        #[qinvokable]
        #[cxx_name = "deleteStock"]
        fn delete_stock(self: Pin<&mut Backend>, id: i64);

        #[qinvokable]
        #[cxx_name = "shopAdd"]
        fn shop_add(self: Pin<&mut Backend>, recipe: i64);
        #[qinvokable]
        #[cxx_name = "shopPortions"]
        fn shop_portions(self: Pin<&mut Backend>, recipe: i64, portions: f64);
        #[qinvokable]
        #[cxx_name = "shopRemove"]
        fn shop_remove(self: Pin<&mut Backend>, recipe: i64);
        #[qinvokable]
        #[cxx_name = "shopPlanned"]
        fn shop_planned(self: Pin<&mut Backend>);
        #[qinvokable]
        #[cxx_name = "shopClear"]
        fn shop_clear(self: Pin<&mut Backend>);
        #[qinvokable]
        #[cxx_name = "shopMarkdown"]
        fn shop_markdown(self: &Backend) -> QString;
        #[qinvokable]
        #[cxx_name = "shopSave"]
        fn shop_save(self: Pin<&mut Backend>, dest: &QString);

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
    }
}

pub struct BackendRust {
    core: Option<Core>,
    recipes: QString,
    tags: QString,
    choices: QString,
    stock: QString,
    calendar: QString,
    shop: QString,
    toast: QString,
    search: QString,
    sort: QString,
    favorites_only: bool,
    min_rating: i32,
    tag_filter: QString,
    filter_count: i32,
    compact: bool,
    today: QString,
    revision: i32,
}

impl Default for BackendRust {
    fn default() -> Self {
        let empty = || QString::from("[]");
        Self {
            core: None,
            recipes: empty(),
            tags: empty(),
            choices: empty(),
            stock: empty(),
            calendar: QString::from("{}"),
            shop: QString::from("{}"),
            toast: QString::default(),
            search: QString::default(),
            sort: QString::from("manual"),
            favorites_only: false,
            min_rating: 0,
            tag_filter: QString::default(),
            filter_count: 0,
            compact: false,
            today: QString::default(),
            revision: 0,
        }
    }
}

fn opt(id: i64) -> Option<i64> {
    (id >= 0).then_some(id)
}

fn sort_of(s: &str) -> Sort {
    match s {
        "title" => Sort::Title,
        "rating" => Sort::Rating,
        "recent" => Sort::Recent,
        _ => Sort::Manual,
    }
}

fn csv(s: &QString) -> Vec<String> {
    s.to_string()
        .split(',')
        .map(|t| t.trim().trim_start_matches(['#', '@']).to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

impl qobject::Backend {
    fn db_path() -> std::path::PathBuf {
        let args: Vec<String> = std::env::args().collect();
        args.iter()
            .position(|a| a == "--db")
            .and_then(|i| args.get(i + 1))
            .map_or_else(
                || organiser_core::paths::db_path(&recipes_schema::META),
                std::path::PathBuf::from,
            )
    }

    pub fn init(mut self: Pin<&mut Self>) {
        match Core::open(&Self::db_path()) {
            Ok(core) => {
                let compact = core.compact();
                self.as_mut().rust_mut().core = Some(core);
                self.as_mut().set_compact(compact);
                self.reload();
            }
            Err(e) => self.set_toast(QString::from(format!("cannot open the database: {e}"))),
        }
    }

    /// Run `f` on the core, then re-publish; an error is shown as the toast.
    fn act(mut self: Pin<&mut Self>, f: impl FnOnce(&mut Core) -> organiser_engine::Result<()>) {
        let result = match self.as_mut().rust_mut().core.as_mut() {
            Some(core) => f(core),
            None => return,
        };
        if let Err(e) = result {
            self.as_mut().set_toast(QString::from(e.to_string()));
        }
        self.reload();
    }

    /// Like [`act`](Self::act), and whether it worked.
    fn act_ok(
        mut self: Pin<&mut Self>,
        f: impl FnOnce(&mut Core) -> organiser_engine::Result<()>,
    ) -> bool {
        let result = match self.as_mut().rust_mut().core.as_mut() {
            Some(core) => f(core),
            None => return false,
        };
        let ok = result.is_ok();
        if let Err(e) = result {
            self.as_mut().set_toast(QString::from(e.to_string()));
        }
        self.reload();
        ok
    }

    /// Show a message the core produced.
    fn say(
        mut self: Pin<&mut Self>,
        f: impl FnOnce(&mut Core) -> organiser_engine::Result<String>,
    ) -> bool {
        let mut said = String::new();
        let ok = self.as_mut().act_ok(|c| {
            said = f(c)?;
            Ok(())
        });
        if ok {
            self.set_toast(QString::from(said));
        }
        ok
    }

    pub fn reload(mut self: Pin<&mut Self>) {
        let published = {
            let mut rust = self.as_mut().rust_mut();
            let Some(core) = rust.core.as_mut() else {
                return;
            };
            let _ = core.refresh();
            let json = |r: organiser_engine::Result<String>| r.unwrap_or_else(|_| "[]".into());
            (
                core.tag_filter.join(","),
                core.filter_count() as i32,
                core.notice.take(),
                json(core.recipes_json()),
                json(core.tags_json()),
                json(core.choices_json()),
                json(core.stock_json()),
                core.calendar_json().unwrap_or_else(|_| "{}".into()),
                core.shop_json(),
                core.today.to_string(),
            )
        };
        let (filter, count, notice, recipes, tags, choices, stock, calendar, shop, today) =
            published;
        if let Some(n) = notice {
            self.as_mut().set_toast(QString::from(n));
        }
        self.as_mut().set_tag_filter(QString::from(filter));
        self.as_mut().set_filter_count(count);
        self.as_mut().set_recipes(QString::from(recipes));
        self.as_mut().set_tags(QString::from(tags));
        self.as_mut().set_choices(QString::from(choices));
        self.as_mut().set_stock(QString::from(stock));
        self.as_mut().set_calendar(QString::from(calendar));
        self.as_mut().set_shop(QString::from(shop));
        self.as_mut().set_today(QString::from(today));
        let revision = self.revision().wrapping_add(1);
        self.as_mut().set_revision(revision);
    }

    pub fn apply_filters(mut self: Pin<&mut Self>) {
        let (search, sort, fav, rating, tags) = (
            self.search().to_string(),
            self.sort().to_string(),
            *self.favorites_only(),
            *self.min_rating(),
            csv(self.tag_filter()),
        );
        if let Some(core) = self.as_mut().rust_mut().core.as_mut() {
            core.text = search;
            core.sort = sort_of(&sort);
            core.favorites = fav;
            core.min_rating = rating.clamp(0, 5) as u8;
            core.tag_filter = tags;
        }
        self.reload();
    }

    pub fn clear_filters(mut self: Pin<&mut Self>) {
        if let Some(core) = self.as_mut().rust_mut().core.as_mut() {
            core.clear_filters();
        }
        self.as_mut().set_favorites_only(false);
        self.as_mut().set_min_rating(0);
        self.reload();
    }

    pub fn save_preferences(mut self: Pin<&mut Self>) {
        let compact = *self.compact();
        let result = match self.as_mut().rust_mut().core.as_mut() {
            Some(core) => core.set_compact(compact),
            None => return,
        };
        if let Err(e) = result {
            self.set_toast(QString::from(e));
        }
    }

    pub fn arg(&self, name: &QString) -> QString {
        let flag = format!("--{name}");
        let args: Vec<String> = std::env::args().collect();
        args.iter()
            .position(|a| *a == flag)
            .and_then(|i| args.get(i + 1))
            .map_or_else(QString::default, |v| QString::from(v.as_str()))
    }

    pub fn recipe(&self, id: i64) -> QString {
        self.core
            .as_ref()
            .and_then(|c| c.recipe_json(id).ok())
            .map_or_else(|| QString::from("{}"), QString::from)
    }

    pub fn meal_times(&self) -> QString {
        QString::from(Core::meal_times_json())
    }

    pub fn create_recipe(mut self: Pin<&mut Self>, title: &QString) -> i64 {
        let title = title.to_string();
        let mut id = -1;
        self.as_mut().act(|c| {
            id = c.create_recipe(&title)?;
            Ok(())
        });
        id
    }

    pub fn delete_recipe(self: Pin<&mut Self>, id: i64) {
        self.act(|c| c.delete_recipe(id));
    }

    pub fn edit_title(self: Pin<&mut Self>, id: i64, title: &QString) {
        let v = title.to_string();
        self.act(|c| c.edit_title(id, &v));
    }

    pub fn edit_notes(self: Pin<&mut Self>, id: i64, notes: &QString) {
        let v = notes.to_string();
        self.act(|c| c.edit_notes(id, &v));
    }

    pub fn edit_servings(self: Pin<&mut Self>, id: i64, servings: &QString) {
        let v = servings.to_string();
        self.act(|c| c.edit_servings(id, &v));
    }

    pub fn set_favorite(self: Pin<&mut Self>, id: i64, on: bool) {
        self.act(|c| c.set_favorite(id, on));
    }

    pub fn set_rating(self: Pin<&mut Self>, id: i64, stars: i32) {
        self.act(|c| c.set_rating(id, stars.clamp(0, 255) as u8));
    }

    pub fn set_recipe_tags(self: Pin<&mut Self>, id: i64, tags: &QString) {
        let list = csv(tags);
        self.act(|c| c.set_tags(id, &list));
    }

    pub fn add_ingredient(
        self: Pin<&mut Self>,
        recipe: i64,
        name: &QString,
        qty: &QString,
        unit: &QString,
        optional: bool,
    ) {
        let (n, q, u) = (name.to_string(), qty.to_string(), unit.to_string());
        self.act(|c| c.add_ingredient(recipe, &n, &q, &u, optional));
    }

    pub fn edit_ingredient(
        self: Pin<&mut Self>,
        id: i64,
        name: &QString,
        qty: &QString,
        unit: &QString,
        optional: bool,
    ) {
        let (n, q, u) = (name.to_string(), qty.to_string(), unit.to_string());
        self.act(|c| c.edit_ingredient(id, &n, &q, &u, optional));
    }

    pub fn move_ingredient(self: Pin<&mut Self>, id: i64, by: i32) {
        self.act(|c| c.move_ingredient(id, by));
    }

    pub fn delete_ingredient(self: Pin<&mut Self>, id: i64) {
        self.act(|c| c.delete_ingredient(id));
    }

    pub fn add_step(self: Pin<&mut Self>, recipe: i64) {
        self.act(|c| c.add_step(recipe));
    }

    pub fn edit_step(self: Pin<&mut Self>, id: i64, text: &QString) {
        let v = text.to_string();
        self.act(|c| c.edit_step(id, &v));
    }

    pub fn move_step(self: Pin<&mut Self>, id: i64, by: i32) {
        self.act(|c| c.move_step(id, by));
    }

    pub fn delete_step(self: Pin<&mut Self>, id: i64) {
        self.act(|c| c.delete_step(id));
    }

    pub fn add_photos(self: Pin<&mut Self>, recipe: i64, step: i64, files: &QString) {
        let list: Vec<String> = files
            .to_string()
            .lines()
            .map(str::to_string)
            .filter(|l| !l.is_empty())
            .collect();
        self.act(|c| c.add_photos(recipe, opt(step), &list));
    }

    pub fn remove_photo(self: Pin<&mut Self>, recipe: i64, step: i64, name: &QString) {
        let n = name.to_string();
        self.act(|c| c.remove_photo(recipe, opt(step), &n));
    }

    pub fn cooked_today(self: Pin<&mut Self>, recipe: i64) {
        self.say(|c| c.cooked_today(recipe));
    }

    pub fn log_meal(
        self: Pin<&mut Self>,
        recipe: i64,
        date: &QString,
        time: &QString,
        servings: &QString,
    ) -> bool {
        let (d, t, s) = (date.to_string(), time.to_string(), servings.to_string());
        self.say(|c| c.log_meal(recipe, &d, &t, &s))
    }

    pub fn delete_meal(self: Pin<&mut Self>, id: i64) {
        self.act(|c| c.delete_meal(id));
    }

    pub fn shift_month(self: Pin<&mut Self>, delta: i32) {
        self.act(|c| {
            c.shift_month(delta);
            Ok(())
        });
    }

    pub fn this_month(self: Pin<&mut Self>) {
        self.act(|c| {
            c.this_month();
            Ok(())
        });
    }

    pub fn add_stock(
        self: Pin<&mut Self>,
        name: &QString,
        qty: &QString,
        unit: &QString,
        expires: &QString,
    ) -> bool {
        let (n, q, u, e) = (
            name.to_string(),
            qty.to_string(),
            unit.to_string(),
            expires.to_string(),
        );
        self.act_ok(|c| c.add_stock(&n, &q, &u, &e))
    }

    pub fn edit_stock(
        self: Pin<&mut Self>,
        id: i64,
        name: &QString,
        qty: &QString,
        unit: &QString,
        expires: &QString,
    ) -> bool {
        let (n, q, u, e) = (
            name.to_string(),
            qty.to_string(),
            unit.to_string(),
            expires.to_string(),
        );
        self.act_ok(|c| c.edit_stock(id, &n, &q, &u, &e))
    }

    pub fn move_stock(self: Pin<&mut Self>, id: i64, by: i32) {
        self.act(|c| c.move_stock(id, by));
    }

    pub fn delete_stock(self: Pin<&mut Self>, id: i64) {
        self.act(|c| c.delete_stock(id));
    }

    pub fn shop_add(self: Pin<&mut Self>, recipe: i64) {
        self.act(|c| c.shop_add(recipe));
    }

    pub fn shop_portions(self: Pin<&mut Self>, recipe: i64, portions: f64) {
        self.act(|c| {
            c.shop_portions(recipe, portions);
            Ok(())
        });
    }

    pub fn shop_remove(self: Pin<&mut Self>, recipe: i64) {
        self.act(|c| {
            c.shop_remove(recipe);
            Ok(())
        });
    }

    pub fn shop_planned(self: Pin<&mut Self>) {
        self.act(|c| c.shop_planned());
    }

    pub fn shop_clear(self: Pin<&mut Self>) {
        self.act(|c| {
            c.shop_clear();
            Ok(())
        });
    }

    pub fn shop_markdown(&self) -> QString {
        self.core
            .as_ref()
            .map_or_else(QString::default, |c| QString::from(c.shop_markdown()))
    }

    pub fn shop_save(self: Pin<&mut Self>, dest: &QString) {
        let d = dest.to_string();
        self.say(|c| c.shop_save(&d).map(|p| format!("Saved {}", p.display())));
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
}
