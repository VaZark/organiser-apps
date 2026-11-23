//! Recipes' state and every action, with no Qt in it: the bridge in `backend.rs` only forwards.
//! Lists come out as JSON that QML reads straight into a list model.

use std::path::{Path, PathBuf};

use chrono::{Datelike, NaiveDate};
use organiser_core::settings::{Loaded, Settings};
use organiser_engine::{ChildPolicy, Db, Error, Result, TagFilter};
use recipes_schema::calendar;
use recipes_schema::lifecycle::{self, NewRecipe};
use recipes_schema::mealtime::MealTime;
use recipes_schema::model::{IngredientEdit, Meal, Recipe, RecipeEdit, StepEdit};
use recipes_schema::shopping::{self, Item};
use recipes_schema::units::pretty;
use recipes_schema::views::{self, Filter, Sort};
use recipes_schema::{SCHEMA, photos};
use serde_json::{Value, json};

/// A recipe picked for the shopping list.
#[derive(Debug, Clone, PartialEq)]
pub struct ShopRow {
    pub id: i64,
    pub title: String,
    pub portions: f64,
    pub servings: Option<f64>,
}

/// What the lists show, and the database behind them.
pub struct Core {
    db: Db,
    pub text: String,
    /// Recipes must have all of these tags (or a tag below one).
    pub tag_filter: Vec<String>,
    pub favorites: bool,
    /// At least this many stars (0: any).
    pub min_rating: u8,
    pub sort: Sort,
    pub today: NaiveDate,
    cal_year: i32,
    cal_month: u32,
    shop: Vec<ShopRow>,
    shop_changed: Option<i64>,
    pub settings: Settings,
    /// What the person should be told after the last action (a settings problem).
    pub notice: Option<String>,
}

fn invalid<T>(msg: &str) -> Result<T> {
    Err(Error::Invalid(msg.into()))
}

/// A quantity as typed: empty is none, a comma counts as a point.
pub fn parse_qty(s: &str) -> Result<Option<f64>> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    match s.replace(',', ".").parse::<f64>() {
        Ok(q) if q.is_finite() && q >= 0.0 => Ok(Some(q)),
        _ => invalid("The quantity is not a number."),
    }
}

/// A date as typed: empty is none.
pub fn parse_date(s: &str) -> Result<Option<NaiveDate>> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    s.parse()
        .map(Some)
        .or_else(|_| invalid("The date should look like 2026-11-30."))
}

fn parse_servings(s: &str) -> Result<Option<f64>> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    match s.parse::<u32>() {
        Ok(n) if n > 0 => Ok(Some(f64::from(n))),
        _ => invalid("Servings should be a whole number, at least 1."),
    }
}

/// The path a `file://` URL names (percent escapes decoded); a plain path stays as it is.
pub fn path_of_url(url: &str) -> PathBuf {
    let Some(rest) = url.strip_prefix("file://") else {
        return PathBuf::from(url);
    };
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(v) = u8::from_str_radix(&rest[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    PathBuf::from(String::from_utf8_lossy(&out).into_owned())
}

fn file_url(path: &Path) -> String {
    let mut out = String::from("file://");
    for b in path.to_string_lossy().bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn photo_json(name: &str) -> Value {
    let url = photos::path(&photos::dir(), name)
        .map(|p| file_url(&p))
        .unwrap_or_default();
    json!({"name": name, "url": url})
}

fn qty_text(q: Option<f64>) -> String {
    q.map(pretty).unwrap_or_default()
}

fn amount_text(a: &Option<(f64, String)>) -> String {
    match a {
        Some((q, u)) if u.is_empty() => pretty(*q),
        Some((q, u)) => format!("{} {u}", pretty(*q)),
        None => String::new(),
    }
}

fn meal_json(m: &Meal, today: NaiveDate) -> Value {
    json!({
        "id": m.id,
        "recipe": m.recipe_id,
        "title": m.title,
        "date": m.date.to_string(),
        "time": m.time.map(|t| t.as_str()).unwrap_or(""),
        "timeLabel": m.time.map(MealTime::label).unwrap_or(""),
        "servings": pretty(m.servings),
        "planned": m.is_planned(today),
    })
}

fn item_json(i: &Item) -> Value {
    use shopping::Status;
    json!({
        "name": i.name,
        "status": match i.status {
            Status::Enough => "enough",
            Status::Missing => "missing",
            Status::Short => "short",
            Status::Unit => "unit",
        },
        "need": amount_text(&i.need),
        "get": amount_text(&i.get),
        "optional": i.optional,
    })
}

impl Core {
    /// Open the database at `path`.
    pub fn open(path: &Path) -> Result<Self> {
        let db = Db::open(path, &SCHEMA)?;
        let Loaded { settings, problem } = Settings::load();
        let today = chrono::Local::now().date_naive();
        let mut core = Self {
            db,
            text: String::new(),
            tag_filter: Vec::new(),
            favorites: false,
            min_rating: 0,
            sort: Sort::Manual,
            today,
            cal_year: today.year(),
            cal_month: today.month(),
            shop: Vec::new(),
            shop_changed: None,
            settings,
            notice: problem,
        };
        core.load_shop();
        Ok(core)
    }

    pub fn compact(&self) -> bool {
        self.settings.recipes.compact
    }

    pub fn set_compact(&mut self, on: bool) -> std::result::Result<(), String> {
        self.settings.recipes.compact = on;
        self.settings.save()
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.today = chrono::Local::now().date_naive();
        let palette = self.settings.palette();
        self.db.ensure_tag_colors(&palette)?;
        // A tag that is gone no longer filters.
        let known: Vec<String> = self.db.tags()?.into_iter().map(|t| t.name).collect();
        self.tag_filter.retain(|t| known.contains(t));
        Ok(())
    }

    fn filter(&self) -> Filter {
        Filter {
            text: (!self.text.trim().is_empty()).then(|| self.text.trim().to_string()),
            tags: TagFilter {
                all: self.tag_filter.clone(),
                ..TagFilter::default()
            },
            favorites: self.favorites,
            min_rating: (self.min_rating > 0).then_some(self.min_rating),
            sort: self.sort,
            ..Filter::default()
        }
    }

    /// How many filters are on (the favourites, a rating, each tag).
    pub fn filter_count(&self) -> usize {
        usize::from(self.favorites) + usize::from(self.min_rating > 0) + self.tag_filter.len()
    }

    pub fn clear_filters(&mut self) {
        self.favorites = false;
        self.min_rating = 0;
        self.tag_filter.clear();
    }

    // ---- reading ---------------------------------------------------------------------------

    pub fn recipes_json(&self) -> Result<String> {
        let counts = views::step_counts(&self.db)?;
        let list: Vec<Value> = views::recipes(&self.db, &self.filter())?
            .iter()
            .map(|r| {
                json!({
                    "id": r.id,
                    "title": r.title,
                    "notes": r.notes,
                    "favorite": r.favorite,
                    "color": r.color.clone().unwrap_or_default(),
                    "rating": r.rating.unwrap_or(0),
                    "servings": qty_text(r.servings),
                    "tags": r.tags,
                    "steps": counts.get(&r.id).copied().unwrap_or(0),
                    "photo": r.photos.first().map(|p| photo_json(p)["url"].clone()).unwrap_or(json!("")),
                })
            })
            .collect();
        Ok(Value::Array(list).to_string())
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

    /// Every recipe by title, for picking one.
    pub fn choices_json(&self) -> Result<String> {
        let list: Vec<Value> = views::recipes(
            &self.db,
            &Filter {
                sort: Sort::Title,
                ..Filter::default()
            },
        )?
        .iter()
        .map(|r| json!({"id": r.id, "title": r.title, "servings": qty_text(r.servings)}))
        .collect();
        Ok(Value::Array(list).to_string())
    }

    /// One recipe with its ingredients, steps, photos and the times it was cooked or planned.
    pub fn recipe_json(&self, id: i64) -> Result<String> {
        let item = self.db.get(id)?;
        // An id freed by a deleted recipe can be reused by another kind of item.
        if item.kind != recipes_schema::RECIPE {
            return Err(Error::NotFound(id));
        }
        let r = Recipe::from(&item);
        let steps: Vec<Value> = views::steps(&self.db, id)?
            .iter()
            .map(|s| {
                json!({
                    "id": s.id,
                    "text": s.text,
                    "photos": s.photos.iter().map(|p| photo_json(p)).collect::<Vec<_>>(),
                })
            })
            .collect();
        let ings: Vec<Value> = views::ingredients(&self.db, id)?
            .iter()
            .map(|i| {
                json!({
                    "id": i.id, "name": i.name, "qty": qty_text(i.quantity),
                    "unit": i.unit, "optional": i.optional,
                })
            })
            .collect();
        let history: Vec<Value> = views::meals_for(&self.db, id)?
            .iter()
            .map(|m| meal_json(m, self.today))
            .collect();
        Ok(json!({
            "id": r.id,
            "title": r.title,
            "notes": r.notes,
            "favorite": r.favorite,
            "color": r.color.clone().unwrap_or_default(),
            "rating": r.rating.unwrap_or(0),
            "servings": qty_text(r.servings),
            "tags": r.tags,
            "photos": r.photos.iter().map(|p| photo_json(p)).collect::<Vec<_>>(),
            "steps": steps,
            "ingredients": ings,
            "history": history,
        })
        .to_string())
    }

    /// The fridge; an item past its date is `expired`.
    pub fn stock_json(&self) -> Result<String> {
        let list: Vec<Value> = views::stock(&self.db)?
            .iter()
            .map(|s| {
                json!({
                    "id": s.id, "name": s.name, "qty": qty_text(s.quantity), "unit": s.unit,
                    "expires": s.expires.map(|d| d.to_string()).unwrap_or_default(),
                    "expired": s.is_expired(self.today),
                })
            })
            .collect();
        Ok(Value::Array(list).to_string())
    }

    pub fn meal_times_json() -> String {
        let list: Vec<Value> = MealTime::ALL
            .iter()
            .map(|t| json!({"value": t.as_str(), "text": t.label()}))
            .collect();
        Value::Array(list).to_string()
    }

    /// The month on show: weeks (Monday first) of days, each with its meals.
    pub fn calendar_json(&self) -> Result<String> {
        let (y, m) = (self.cal_year, self.cal_month);
        let weeks = calendar::weeks(y, m);
        let (from, to) = match (weeks.first(), weeks.last()) {
            (Some(f), Some(l)) => (f[0], l[6]),
            _ => calendar::bounds(y, m),
        };
        let meals = views::meals_between(&self.db, from, to)?;
        let weeks: Vec<Value> = weeks
            .iter()
            .map(|w| {
                Value::Array(
                    w.iter()
                        .map(|d| {
                            let of_day: Vec<Value> = meals
                                .iter()
                                .filter(|m| m.date == *d)
                                .map(|m| meal_json(m, self.today))
                                .collect();
                            json!({
                                "date": d.to_string(),
                                "day": d.day(),
                                "inMonth": d.month() == m,
                                "today": *d == self.today,
                                "meals": of_day,
                            })
                        })
                        .collect(),
                )
            })
            .collect();
        Ok(json!({
            "year": y,
            "month": m,
            "title": format!("{} {y}", calendar::month_name(m)),
            "weeks": weeks,
        })
        .to_string())
    }

    pub fn shift_month(&mut self, delta: i32) {
        (self.cal_year, self.cal_month) = calendar::shift(self.cal_year, self.cal_month, delta);
    }

    pub fn this_month(&mut self) {
        self.cal_year = self.today.year();
        self.cal_month = self.today.month();
    }

    // ---- recipes ---------------------------------------------------------------------------

    pub fn create_recipe(&mut self, title: &str) -> Result<i64> {
        if title.trim().is_empty() {
            return invalid("Give the recipe a title first.");
        }
        Ok(lifecycle::add_recipe(&mut self.db, &NewRecipe::new(title.trim()))?.id)
    }

    fn edit(&mut self, id: i64, edit: RecipeEdit) -> Result<()> {
        lifecycle::edit_recipe(&mut self.db, id, &edit)?;
        Ok(())
    }

    pub fn edit_title(&mut self, id: i64, title: &str) -> Result<()> {
        if title.trim().is_empty() {
            return invalid("A recipe needs a title.");
        }
        self.edit(
            id,
            RecipeEdit {
                title: Some(title.trim().into()),
                ..Default::default()
            },
        )
    }

    pub fn edit_notes(&mut self, id: i64, notes: &str) -> Result<()> {
        self.edit(
            id,
            RecipeEdit {
                notes: Some(notes.trim_end().into()),
                ..Default::default()
            },
        )
    }

    pub fn edit_servings(&mut self, id: i64, servings: &str) -> Result<()> {
        let v = parse_servings(servings)?;
        self.edit(
            id,
            RecipeEdit {
                servings: Some(v),
                ..Default::default()
            },
        )
    }

    pub fn set_favorite(&mut self, id: i64, on: bool) -> Result<()> {
        lifecycle::set_favorite(&mut self.db, id, on)?;
        Ok(())
    }

    /// `stars` 0 clears the rating.
    pub fn set_rating(&mut self, id: i64, stars: u8) -> Result<()> {
        if stars > 5 {
            return invalid("A rating is 0 to 5 stars.");
        }
        self.edit(
            id,
            RecipeEdit {
                rating: Some((stars > 0).then_some(stars)),
                ..Default::default()
            },
        )
    }

    /// `#rrggbb` (or `#rgb`), or empty for the default card.
    pub fn set_color(&mut self, id: i64, color: &str) -> Result<()> {
        let c = organiser_core::color::parse_input(color).map_err(Error::Invalid)?;
        self.edit(
            id,
            RecipeEdit {
                color: Some(c.map(|c| c.hex())),
                ..Default::default()
            },
        )
    }

    pub fn set_tags(&mut self, id: i64, tags: &[String]) -> Result<()> {
        lifecycle::set_tags(&mut self.db, id, tags)
    }

    pub fn delete_recipe(&mut self, id: i64) -> Result<()> {
        self.db.delete(id)?;
        let _ = photos::gc(&self.db, &photos::dir());
        // The shopping list forgets a recipe that is gone.
        self.shop.retain(|r| r.id != id);
        self.rebuild_shop();
        Ok(())
    }

    // ---- ingredients and steps -------------------------------------------------------------

    pub fn add_ingredient(
        &mut self,
        recipe: i64,
        name: &str,
        qty: &str,
        unit: &str,
        optional: bool,
    ) -> Result<()> {
        let q = parse_qty(qty)?;
        lifecycle::add_ingredient(&mut self.db, recipe, name, q, unit, optional)?;
        Ok(())
    }

    pub fn edit_ingredient(
        &mut self,
        id: i64,
        name: &str,
        qty: &str,
        unit: &str,
        optional: bool,
    ) -> Result<()> {
        let q = parse_qty(qty)?;
        lifecycle::edit_ingredient(
            &mut self.db,
            id,
            &IngredientEdit {
                name: Some(name.into()),
                quantity: Some(q),
                unit: Some(unit.into()),
                optional: Some(optional),
            },
        )?;
        Ok(())
    }

    pub fn move_ingredient(&mut self, id: i64, by: i32) -> Result<()> {
        lifecycle::move_ingredient(&mut self.db, id, by)
    }

    pub fn delete_ingredient(&mut self, id: i64) -> Result<()> {
        lifecycle::delete_ingredient(&mut self.db, id)
    }

    pub fn add_step(&mut self, recipe: i64) -> Result<()> {
        lifecycle::add_step(&mut self.db, recipe, "")?;
        Ok(())
    }

    pub fn edit_step(&mut self, id: i64, text: &str) -> Result<()> {
        lifecycle::edit_step(
            &mut self.db,
            id,
            &StepEdit {
                text: Some(text.trim_end().into()),
                photos: None,
            },
        )?;
        Ok(())
    }

    pub fn move_step(&mut self, id: i64, by: i32) -> Result<()> {
        lifecycle::move_step(&mut self.db, id, by)
    }

    pub fn delete_step(&mut self, id: i64) -> Result<()> {
        lifecycle::delete_step(&mut self.db, id)?;
        let _ = photos::gc(&self.db, &photos::dir());
        Ok(())
    }

    // ---- photos ----------------------------------------------------------------------------

    /// The photo names of the recipe (`step` is `None`) or of one of its steps.
    fn photo_names(&self, recipe: i64, step: Option<i64>) -> Result<Vec<String>> {
        Ok(match step {
            None => Recipe::from(&self.db.get(recipe)?).photos,
            Some(s) => views::steps(&self.db, recipe)?
                .into_iter()
                .find(|x| x.id == s)
                .map(|x| x.photos)
                .ok_or(Error::NotFound(s))?,
        })
    }

    fn save_photos(&mut self, recipe: i64, step: Option<i64>, list: Vec<String>) -> Result<()> {
        match step {
            None => {
                lifecycle::edit_recipe(
                    &mut self.db,
                    recipe,
                    &RecipeEdit {
                        photos: Some(list),
                        ..Default::default()
                    },
                )?;
            }
            Some(s) => {
                lifecycle::edit_step(
                    &mut self.db,
                    s,
                    &StepEdit {
                        text: None,
                        photos: Some(list),
                    },
                )?;
            }
        }
        let _ = photos::gc(&self.db, &photos::dir());
        Ok(())
    }

    /// Add pictures (paths or `file://` URLs) to the recipe or one of its steps.
    pub fn add_photos(&mut self, recipe: i64, step: Option<i64>, files: &[String]) -> Result<()> {
        self.add_photos_in(&photos::dir(), recipe, step, files)
    }

    fn add_photos_in(
        &mut self,
        dir: &Path,
        recipe: i64,
        step: Option<i64>,
        files: &[String],
    ) -> Result<()> {
        let mut list = self.photo_names(recipe, step)?;
        let mut first_error = None;
        for f in files {
            match photos::import(dir, &path_of_url(f)) {
                Ok(name) => {
                    if !list.contains(&name) {
                        list.push(name);
                    }
                }
                Err(e) => first_error = first_error.or(Some(e)),
            }
        }
        self.save_photos(recipe, step, list)?;
        first_error.map_or(Ok(()), Err)
    }

    pub fn remove_photo(&mut self, recipe: i64, step: Option<i64>, name: &str) -> Result<()> {
        let mut list = self.photo_names(recipe, step)?;
        list.retain(|p| p != name);
        self.save_photos(recipe, step, list)
    }

    // ---- meals -----------------------------------------------------------------------------

    /// Log the recipe as cooked today, at its own servings.
    pub fn cooked_today(&mut self, recipe: i64) -> Result<String> {
        let servings = Recipe::from(&self.db.get(recipe)?).servings.unwrap_or(1.0);
        let m = lifecycle::add_meal(&mut self.db, recipe, self.today, None, servings, "")?;
        Ok(format!("Logged {} for today", m.title))
    }

    /// Put a recipe on a day (`time` empty: any time of day).
    pub fn log_meal(
        &mut self,
        recipe: i64,
        date: &str,
        time: &str,
        servings: &str,
    ) -> Result<String> {
        let Some(day) = parse_date(date)? else {
            return invalid("The date should look like 2026-11-30.");
        };
        let servings = match parse_qty(servings) {
            Ok(Some(n)) if n >= 1.0 => n,
            _ => return invalid("Servings should be a number, at least 1."),
        };
        let time = MealTime::parse(time);
        let m = lifecycle::add_meal(&mut self.db, recipe, day, time, servings, "")?;
        Ok(format!("Logged {}", m.title))
    }

    pub fn delete_meal(&mut self, id: i64) -> Result<()> {
        lifecycle::delete_meal(&mut self.db, id)
    }

    // ---- the fridge ------------------------------------------------------------------------

    pub fn add_stock(&mut self, name: &str, qty: &str, unit: &str, expires: &str) -> Result<()> {
        let (q, d) = (parse_qty(qty)?, parse_date(expires)?);
        lifecycle::add_stock(&mut self.db, name, q, unit, d)?;
        Ok(())
    }

    pub fn edit_stock(
        &mut self,
        id: i64,
        name: &str,
        qty: &str,
        unit: &str,
        expires: &str,
    ) -> Result<()> {
        let (q, d) = (parse_qty(qty)?, parse_date(expires)?);
        if !name.trim().is_empty() {
            lifecycle::rename_stock(&mut self.db, id, name)?;
        }
        lifecycle::edit_stock(&mut self.db, id, Some(q), Some(unit), Some(d))?;
        Ok(())
    }

    pub fn move_stock(&mut self, id: i64, by: i32) -> Result<()> {
        lifecycle::move_stock(&mut self.db, id, by)
    }

    pub fn delete_stock(&mut self, id: i64) -> Result<()> {
        lifecycle::delete_stock(&mut self.db, id)
    }

    // ---- the shopping list -----------------------------------------------------------------

    fn times(&self) -> Vec<(i64, f64)> {
        self.shop
            .iter()
            .map(|r| (r.id, views::times_for(r.servings, r.portions)))
            .collect()
    }

    fn load_shop(&mut self) {
        let Ok(list) = views::shopping_picks(&self.db) else {
            return;
        };
        self.shop_changed = list.updated;
        self.shop = list
            .picks
            .into_iter()
            .filter_map(|(id, portions)| {
                let r = Recipe::from(&self.db.get(id).ok()?);
                Some(ShopRow {
                    id,
                    title: r.title,
                    portions,
                    servings: r.servings,
                })
            })
            .collect();
    }

    /// Remember the picks and the day they last changed.
    fn rebuild_shop(&mut self) {
        let saved: Vec<(i64, f64)> = self.shop.iter().map(|r| (r.id, r.portions)).collect();
        if let Ok(list) = lifecycle::save_shopping(&mut self.db, &saved) {
            self.shop_changed = list.updated;
        }
    }

    fn items(&self) -> Vec<Item> {
        views::shopping_list(&self.db, &self.times(), self.today, false).unwrap_or_default()
    }

    pub fn shop_json(&self) -> String {
        let items = self.items();
        json!({
            "picks": self.shop.iter().map(|r| json!({
                "id": r.id, "title": r.title, "portions": pretty(r.portions),
                "servings": qty_text(r.servings),
            })).collect::<Vec<_>>(),
            "items": items.iter().map(item_json).collect::<Vec<_>>(),
            "canMake": !self.shop.is_empty() && shopping::can_make(&items),
            "changed": self.shop_changed,
        })
        .to_string()
    }

    pub fn shop_markdown(&self) -> String {
        views::shopping_markdown(&self.items())
    }

    pub fn shop_add(&mut self, recipe: i64) -> Result<()> {
        let r = Recipe::from(&self.db.get(recipe)?);
        match self.shop.iter_mut().find(|x| x.id == recipe) {
            Some(row) => row.portions += 1.0,
            None => self.shop.push(ShopRow {
                id: recipe,
                title: r.title,
                portions: r.servings.unwrap_or(1.0),
                servings: r.servings,
            }),
        }
        self.rebuild_shop();
        Ok(())
    }

    pub fn shop_portions(&mut self, recipe: i64, portions: f64) {
        if let Some(r) = self.shop.iter_mut().find(|x| x.id == recipe) {
            r.portions = portions.max(1.0);
        }
        self.rebuild_shop();
    }

    pub fn shop_remove(&mut self, recipe: i64) {
        self.shop.retain(|r| r.id != recipe);
        self.rebuild_shop();
    }

    /// Add every meal planned after today, with its servings.
    pub fn shop_planned(&mut self) -> Result<()> {
        let from = self.today + chrono::Days::new(1);
        let to = NaiveDate::from_ymd_opt(2100, 12, 31).expect("a date");
        for m in views::meals_between(&self.db, from, to)? {
            let Some(id) = m.recipe_id else { continue };
            let Ok(item) = self.db.get(id) else { continue };
            match self.shop.iter_mut().find(|r| r.id == id) {
                Some(r) => r.portions += m.servings,
                None => self.shop.push(ShopRow {
                    id,
                    title: m.title,
                    portions: m.servings,
                    servings: Recipe::from(&item).servings,
                }),
            }
        }
        self.rebuild_shop();
        Ok(())
    }

    pub fn shop_clear(&mut self) {
        self.shop.clear();
        self.rebuild_shop();
    }

    /// Write the list as Markdown task lines to a file.
    pub fn shop_save(&self, dest: &str) -> Result<PathBuf> {
        let path = path_of_url(dest);
        std::fs::write(&path, self.shop_markdown() + "\n")?;
        Ok(path)
    }

    // ---- tags ------------------------------------------------------------------------------

    pub fn create_tag(&mut self, name: &str, parent: &str) -> Result<()> {
        self.db
            .create_tag(name.trim(), (!parent.is_empty()).then_some(parent))?;
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "recipes-kirigami-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn core() -> Core {
        Core::open(&temp("db").join("t.db")).unwrap()
    }

    fn parse(s: String) -> Value {
        serde_json::from_str(&s).unwrap()
    }

    /// A recipe with two ingredients, made for 2.
    fn pasta(c: &mut Core) -> i64 {
        let id = c.create_recipe("Pasta").unwrap();
        c.edit_servings(id, "2").unwrap();
        c.add_ingredient(id, "Pasta", "200", "g", false).unwrap();
        c.add_ingredient(id, "Basil", "", "", true).unwrap();
        id
    }

    #[test]
    fn parsing_what_was_typed() {
        assert_eq!(parse_qty(" 1,5 ").unwrap(), Some(1.5));
        assert_eq!(parse_qty("").unwrap(), None);
        assert!(parse_qty("lots").is_err());
        assert!(parse_qty("-1").is_err());
        assert!(parse_date("2026-13-01").is_err());
        assert_eq!(parse_date("").unwrap(), None);
        assert!(parse_servings("0").is_err());
        assert_eq!(parse_servings("4").unwrap(), Some(4.0));
    }

    #[test]
    fn urls_become_paths_and_back() {
        assert_eq!(
            path_of_url("file:///home/a%20b/p%C3%A9.png"),
            PathBuf::from("/home/a b/pé.png")
        );
        assert_eq!(path_of_url("/x/y.png"), PathBuf::from("/x/y.png"));
        assert_eq!(file_url(Path::new("/a b/c.png")), "file:///a%20b/c.png");
    }

    #[test]
    fn recipe_edit_and_delete() {
        let mut c = core();
        assert!(c.create_recipe("  ").is_err());
        let id = pasta(&mut c);
        c.edit_title(id, "Pasta al pesto").unwrap();
        assert!(c.edit_title(id, " ").is_err());
        c.edit_notes(id, "Boil.  \n").unwrap();
        c.set_favorite(id, true).unwrap();
        c.set_rating(id, 4).unwrap();
        assert!(c.set_rating(id, 9).is_err());
        c.set_tags(id, &["dinner".to_string()]).unwrap();
        let v = parse(c.recipe_json(id).unwrap());
        assert_eq!(v["title"], "Pasta al pesto");
        assert_eq!(v["notes"], "Boil.");
        assert_eq!(v["favorite"], true);
        assert_eq!(v["rating"], 4);
        assert_eq!(v["servings"], "2");
        assert_eq!(v["tags"][0], "dinner");
        assert_eq!(v["ingredients"].as_array().unwrap().len(), 2);
        c.set_rating(id, 0).unwrap();
        assert_eq!(parse(c.recipe_json(id).unwrap())["rating"], 0);
        assert_eq!(parse(c.recipe_json(id).unwrap())["color"], "");
        c.set_color(id, "#F80").unwrap();
        assert_eq!(parse(c.recipe_json(id).unwrap())["color"], "#ff8800");
        assert!(c.set_color(id, "red").is_err());
        assert_eq!(parse(c.recipe_json(id).unwrap())["color"], "#ff8800");
        c.set_color(id, "").unwrap();
        assert_eq!(parse(c.recipe_json(id).unwrap())["color"], "");
        c.delete_recipe(id).unwrap();
        assert!(c.recipe_json(id).is_err());
    }

    #[test]
    fn filters_narrow_the_list() {
        let mut c = core();
        let a = c.create_recipe("Soup").unwrap();
        let b = c.create_recipe("Cake").unwrap();
        c.set_favorite(a, true).unwrap();
        c.set_rating(b, 5).unwrap();
        c.set_tags(b, &["sweet".to_string()]).unwrap();
        let titles = |c: &Core| -> Vec<String> {
            parse(c.recipes_json().unwrap())
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["title"].as_str().unwrap().to_string())
                .collect()
        };
        assert_eq!(titles(&c).len(), 2);
        c.favorites = true;
        assert_eq!(titles(&c), ["Soup"]);
        assert_eq!(c.filter_count(), 1);
        c.clear_filters();
        c.min_rating = 4;
        assert_eq!(titles(&c), ["Cake"]);
        c.clear_filters();
        c.tag_filter = vec!["sweet".into()];
        assert_eq!(titles(&c), ["Cake"]);
        c.clear_filters();
        c.text = "soup".into();
        assert_eq!(titles(&c), ["Soup"]);
        c.text.clear();
        c.sort = Sort::Title;
        assert_eq!(titles(&c), ["Cake", "Soup"]);
        // A deleted tag stops filtering.
        c.tag_filter = vec!["sweet".into()];
        c.delete_tag("sweet", false).unwrap();
        c.refresh().unwrap();
        assert!(c.tag_filter.is_empty());
    }

    #[test]
    fn steps_and_ingredients_move() {
        let mut c = core();
        let id = pasta(&mut c);
        c.add_step(id).unwrap();
        c.add_step(id).unwrap();
        let v = parse(c.recipe_json(id).unwrap());
        let (s1, s2) = (
            v["steps"][0]["id"].as_i64().unwrap(),
            v["steps"][1]["id"].as_i64().unwrap(),
        );
        c.edit_step(s1, "Boil water").unwrap();
        c.move_step(s1, 1).unwrap();
        let v = parse(c.recipe_json(id).unwrap());
        assert_eq!(v["steps"][0]["id"].as_i64().unwrap(), s2);
        assert_eq!(v["steps"][1]["text"], "Boil water");
        c.delete_step(s2).unwrap();
        let i = v["ingredients"][0]["id"].as_i64().unwrap();
        c.edit_ingredient(i, "Spaghetti", "250", "g", true).unwrap();
        assert!(c.edit_ingredient(i, "x", "many", "", false).is_err());
        c.move_ingredient(i, 1).unwrap();
        let v = parse(c.recipe_json(id).unwrap());
        assert_eq!(v["ingredients"][1]["name"], "Spaghetti");
        assert_eq!(v["ingredients"][1]["optional"], true);
        assert_eq!(v["steps"].as_array().unwrap().len(), 1);
        assert!(c.add_ingredient(id, "Salt", "x", "", false).is_err());
    }

    #[test]
    fn photos_are_imported_and_removed() {
        let mut c = core();
        let id = c.create_recipe("Pie").unwrap();
        let dir = temp("photos");
        let src = dir.join("pie.png");
        std::fs::write(&src, b"\x89PNG not really").unwrap();
        let url = format!("file://{}", src.display());
        c.add_photos_in(&dir.join("store"), id, None, &[url])
            .unwrap();
        let v = parse(c.recipe_json(id).unwrap());
        let name = v["photos"][0]["name"].as_str().unwrap().to_string();
        // A file that is no picture is refused, and the good ones still land.
        let bad = dir.join("a.txt");
        std::fs::write(&bad, "x").unwrap();
        assert!(
            c.add_photos_in(&dir.join("store"), id, None, &[bad.display().to_string()])
                .is_err()
        );
        c.remove_photo(id, None, &name).unwrap();
        let v = parse(c.recipe_json(id).unwrap());
        assert!(v["photos"].as_array().unwrap().is_empty());
    }

    #[test]
    fn meals_show_on_the_calendar_and_in_history() {
        let mut c = core();
        let id = pasta(&mut c);
        c.today = NaiveDate::from_ymd_opt(2026, 11, 23).unwrap();
        c.this_month();
        let msg = c.cooked_today(id).unwrap();
        assert_eq!(msg, "Logged Pasta for today");
        c.log_meal(id, "2026-11-30", "dinner", "4").unwrap();
        assert!(c.log_meal(id, "soon", "", "1").is_err());
        assert!(c.log_meal(id, "2026-11-30", "", "0").is_err());
        let cal = parse(c.calendar_json().unwrap());
        assert_eq!(cal["title"], "November 2026");
        let days: Vec<&Value> = cal["weeks"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|w| w.as_array().unwrap())
            .collect();
        let today = days.iter().find(|d| d["date"] == "2026-11-23").unwrap();
        assert_eq!(today["today"], true);
        assert_eq!(today["meals"][0]["planned"], false);
        let planned = days.iter().find(|d| d["date"] == "2026-11-30").unwrap();
        assert_eq!(planned["meals"][0]["planned"], true);
        assert_eq!(planned["meals"][0]["timeLabel"], "Dinner");
        c.shift_month(1);
        assert_eq!(parse(c.calendar_json().unwrap())["title"], "December 2026");
        let h = parse(c.recipe_json(id).unwrap());
        assert_eq!(h["history"].as_array().unwrap().len(), 2);
        let m = planned["meals"][0]["id"].as_i64().unwrap();
        c.delete_meal(m).unwrap();
        assert_eq!(
            parse(c.recipe_json(id).unwrap())["history"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn the_fridge_is_kept_and_expires() {
        let mut c = core();
        c.today = NaiveDate::from_ymd_opt(2026, 11, 23).unwrap();
        c.add_stock("Milk", "1", "l", "2026-11-01").unwrap();
        c.add_stock("Rice", "500", "g", "").unwrap();
        assert!(c.add_stock("Eggs", "6", "", "tomorrow").is_err());
        let s = parse(c.stock_json().unwrap());
        assert_eq!(s[0]["expired"], true);
        assert_eq!(s[1]["expired"], false);
        let rice = s[1]["id"].as_i64().unwrap();
        c.edit_stock(rice, "Basmati rice", "250", "g", "").unwrap();
        c.move_stock(rice, -1).unwrap();
        let s = parse(c.stock_json().unwrap());
        assert_eq!(s[0]["name"], "Basmati rice");
        assert_eq!(s[0]["qty"], "250");
        c.delete_stock(rice).unwrap();
        assert_eq!(parse(c.stock_json().unwrap()).as_array().unwrap().len(), 1);
    }

    #[test]
    fn the_shopping_list_is_remembered_and_set_against_the_fridge() {
        let path = temp("shop").join("t.db");
        let mut c = Core::open(&path).unwrap();
        c.today = NaiveDate::from_ymd_opt(2026, 11, 23).unwrap();
        let id = pasta(&mut c);
        c.shop_add(id).unwrap();
        // Two portions of a recipe for two: once. Adding it again adds a portion.
        c.shop_portions(id, 4.0);
        c.add_stock("Pasta", "100", "g", "").unwrap();
        let s = parse(c.shop_json());
        assert_eq!(s["picks"][0]["portions"], "4");
        let pasta_item = &s["items"][0];
        assert_eq!(pasta_item["name"], "Pasta");
        assert_eq!(pasta_item["status"], "short");
        assert_eq!(pasta_item["get"], "300 g");
        assert_eq!(s["canMake"], false);
        assert_eq!(c.shop_markdown(), "- [ ] 300 g Pasta");
        let out = temp("out").join("list.md");
        c.shop_save(&out.display().to_string()).unwrap();
        assert_eq!(
            std::fs::read_to_string(&out).unwrap(),
            "- [ ] 300 g Pasta\n"
        );
        // Reopened: the picks are back.
        drop(c);
        let mut c = Core::open(&path).unwrap();
        assert_eq!(parse(c.shop_json())["picks"][0]["id"], id);
        // Planned meals add their servings.
        c.today = NaiveDate::from_ymd_opt(2026, 11, 23).unwrap();
        c.shop_clear();
        c.log_meal(id, "2026-12-01", "", "3").unwrap();
        c.shop_planned().unwrap();
        assert_eq!(parse(c.shop_json())["picks"][0]["portions"], "3");
        c.shop_remove(id);
        assert!(parse(c.shop_json())["picks"].as_array().unwrap().is_empty());
        // A recipe that is deleted leaves the list.
        c.shop_add(id).unwrap();
        c.delete_recipe(id).unwrap();
        assert!(parse(c.shop_json())["picks"].as_array().unwrap().is_empty());
    }

    #[test]
    fn tags_have_colours_and_parents() {
        let mut c = core();
        c.create_tag("dinner", "").unwrap();
        c.create_tag("pasta", "dinner").unwrap();
        c.set_tag_color("pasta", "#ff0000").unwrap();
        let t = parse(c.tags_json().unwrap());
        let pasta = t
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == "pasta")
            .unwrap();
        assert_eq!(pasta["parent"], "dinner");
        assert_eq!(pasta["color"], "#ff0000");
        c.set_tag_parent("pasta", "").unwrap();
        c.delete_tag("dinner", false).unwrap();
        assert_eq!(parse(c.tags_json().unwrap()).as_array().unwrap().len(), 1);
    }
}
