//! Recipes, steps, ingredients, the fridge and meals as typed values, and the patches that edit them.

use chrono::NaiveDate;
use organiser_engine::{Item, ItemPatch};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::mealtime::MealTime;

fn text(i: &Item, key: &str) -> String {
    i.fields
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// A list of strings (photo names), skipping anything else.
fn strings(i: &Item, key: &str) -> Vec<String> {
    i.fields
        .get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

fn flag(i: &Item, key: &str) -> bool {
    i.fields.get(key).and_then(Value::as_bool).unwrap_or(false)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recipe {
    pub id: i64,
    pub title: String,
    /// The person's own note on the whole recipe.
    pub notes: String,
    /// Photo file names (see `photo`).
    pub photos: Vec<String>,
    /// Portions it makes.
    pub servings: Option<f64>,
    pub favorite: bool,
    /// 1 to 5 stars.
    pub rating: Option<u8>,
    pub tags: Vec<String>,
    pub position: i64,
    pub created: i64,
    pub updated: i64,
}

impl From<&Item> for Recipe {
    fn from(i: &Item) -> Self {
        Self {
            id: i.id,
            title: i.title.clone(),
            notes: text(i, "notes"),
            photos: strings(i, "photos"),
            servings: i.fields.get("servings").and_then(Value::as_f64),
            favorite: flag(i, "favorite"),
            rating: i
                .fields
                .get("rating")
                .and_then(Value::as_u64)
                .map(|r| r as u8),
            tags: i.tags.clone(),
            position: i.position,
            created: i.created,
            updated: i.updated,
        }
    }
}

/// A change to a recipe. `None` leaves a field alone; `Some(None)` clears a rating.
#[derive(Debug, Clone, Default)]
pub struct RecipeEdit {
    pub title: Option<String>,
    pub notes: Option<String>,
    pub photos: Option<Vec<String>>,
    /// `Some(None)` clears it.
    pub servings: Option<Option<f64>>,
    pub favorite: Option<bool>,
    pub rating: Option<Option<u8>>,
}

impl RecipeEdit {
    pub fn to_patch(&self) -> ItemPatch {
        let mut p = ItemPatch {
            title: self.title.clone(),
            ..ItemPatch::default()
        };
        if let Some(n) = &self.notes {
            p.fields.insert(
                "notes".into(),
                if n.is_empty() {
                    Value::Null
                } else {
                    n.clone().into()
                },
            );
        }
        if let Some(photos) = &self.photos {
            p.fields.insert(
                "photos".into(),
                if photos.is_empty() {
                    Value::Null
                } else {
                    photos.clone().into()
                },
            );
        }
        if let Some(sv) = self.servings {
            p.fields
                .insert("servings".into(), sv.map_or(Value::Null, Value::from));
        }
        if let Some(f) = self.favorite {
            p.fields.insert("favorite".into(), f.into());
        }
        if let Some(r) = self.rating {
            p.fields
                .insert("rating".into(), r.map_or(Value::Null, Value::from));
        }
        p
    }
}

/// One step of a recipe. Steps are ordered by `position`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub id: i64,
    pub recipe_id: i64,
    /// Markdown, any number of lines.
    pub text: String,
    pub photos: Vec<String>,
    pub position: i64,
}

impl From<&Item> for Step {
    fn from(i: &Item) -> Self {
        Self {
            id: i.id,
            recipe_id: i.parent_id.unwrap_or_default(),
            text: text(i, "text"),
            photos: strings(i, "photos"),
            position: i.position,
        }
    }
}

/// A change to a step. `None` leaves a field alone.
#[derive(Debug, Clone, Default)]
pub struct StepEdit {
    pub text: Option<String>,
    pub photos: Option<Vec<String>>,
}

/// One ingredient a recipe needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ingredient {
    pub id: i64,
    pub recipe_id: i64,
    pub name: String,
    /// `None` for "to taste": only needs to be there.
    pub quantity: Option<f64>,
    /// `g`, `ml`, `tbsp`, `pcs`… or empty for a plain count.
    pub unit: String,
    pub optional: bool,
    pub position: i64,
}

impl From<&Item> for Ingredient {
    fn from(i: &Item) -> Self {
        Self {
            id: i.id,
            recipe_id: i.parent_id.unwrap_or_default(),
            name: i.title.clone(),
            quantity: i.fields.get("quantity").and_then(Value::as_f64),
            unit: text(i, "unit"),
            optional: flag(i, "optional"),
            position: i.position,
        }
    }
}

/// A change to an ingredient. `None` leaves a field alone; `Some(None)` clears the quantity.
#[derive(Debug, Clone, Default)]
pub struct IngredientEdit {
    pub name: Option<String>,
    pub quantity: Option<Option<f64>>,
    pub unit: Option<String>,
    pub optional: Option<bool>,
}

/// Something in the fridge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stock {
    pub id: i64,
    pub name: String,
    pub quantity: Option<f64>,
    pub unit: String,
    /// The last day it is good, if it has one.
    pub expires: Option<NaiveDate>,
}

impl Stock {
    /// Past its date on `today`.
    pub fn is_expired(&self, today: NaiveDate) -> bool {
        self.expires.is_some_and(|e| e < today)
    }
}

impl From<&Item> for Stock {
    fn from(i: &Item) -> Self {
        Self {
            id: i.id,
            name: i.title.clone(),
            quantity: i.fields.get("quantity").and_then(Value::as_f64),
            unit: text(i, "unit"),
            expires: text(i, "expires").parse().ok(),
        }
    }
}

/// A recipe on a day: cooked if the day is up to today, planned if it is later.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Meal {
    pub id: i64,
    /// The recipe, while it still exists.
    pub recipe_id: Option<i64>,
    /// The recipe's title when the meal was added.
    pub title: String,
    pub date: NaiveDate,
    /// When in the day, if said.
    pub time: Option<MealTime>,
    /// How many servings (meal prep for five is 5). At least 1.
    pub servings: f64,
    pub note: String,
}

impl Meal {
    pub fn is_planned(&self, today: NaiveDate) -> bool {
        self.date > today
    }
}

impl From<&Item> for Meal {
    fn from(i: &Item) -> Self {
        Self {
            id: i.id,
            recipe_id: i.fields.get("recipe").and_then(Value::as_i64),
            title: i.title.clone(),
            date: text(i, "date")
                .parse()
                .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).expect("a valid date")),
            time: i
                .fields
                .get("time")
                .and_then(Value::as_str)
                .and_then(MealTime::parse),
            servings: i
                .fields
                .get("servings")
                .and_then(Value::as_f64)
                .filter(|t| *t >= 1.0)
                .unwrap_or(1.0),
            note: text(i, "note"),
        }
    }
}

/// The shopping list being prepared: recipes with the portions wanted of each, and when it was
/// last changed (seconds since the epoch), if it ever was.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ShopList {
    pub picks: Vec<(i64, f64)>,
    pub updated: Option<i64>,
}

impl ShopList {
    pub fn from_item(i: &Item) -> Self {
        let picks = strings(i, "picks")
            .iter()
            .filter_map(|p| {
                let (id, portions) = p.split_once(':')?;
                Some((id.parse().ok()?, portions.parse().ok()?))
            })
            .collect();
        Self {
            picks,
            updated: Some(i.updated),
        }
    }
}
