//! Creating and editing recipes, their steps, and the cooking log.

use chrono::NaiveDate;
use organiser_engine::{Db, Error, ItemPatch, NewItem, Result};
use serde_json::Value;

use crate::mealtime::MealTime;
use crate::primitives::model::{
    Ingredient, IngredientEdit, Meal, Recipe, RecipeEdit, ShopList, Step, StepEdit, Stock,
};
use crate::primitives::{photo, step::title_of};
use crate::{INGREDIENT, MEAL, RECIPE, SHOPLIST, STEP, STOCK};

#[derive(Debug, Clone, Default)]
pub struct NewRecipe {
    pub title: String,
    pub notes: Option<String>,
    pub photos: Vec<String>,
    pub servings: Option<f64>,
    pub favorite: bool,
    pub rating: Option<u8>,
    pub tags: Vec<String>,
}

impl NewRecipe {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.into(),
            ..Self::default()
        }
    }
}

/// Add a recipe.
pub fn add_recipe(db: &mut Db, e: &NewRecipe) -> Result<Recipe> {
    let mut new = NewItem::new(RECIPE, &e.title);
    new.tags = e.tags.clone();
    if let Some(n) = &e.notes {
        new = new.field("notes", n.clone());
    }
    if !e.photos.is_empty() {
        check_photos(&e.photos)?;
        new = new.field("photos", e.photos.clone());
    }
    if let Some(sv) = e.servings {
        new = new.field("servings", positive(sv)?);
    }
    if e.favorite {
        new = new.field("favorite", true);
    }
    if let Some(r) = e.rating {
        new = new.field("rating", r);
    }
    Ok(Recipe::from(&db.create(new)?))
}

/// Photo names must be ones this app made: nothing else is ever opened.
fn check_photos(photos: &[String]) -> Result<()> {
    match photos.iter().find(|p| !photo::is_valid_name(p)) {
        Some(bad) => Err(Error::Invalid(format!(
            "`{bad}` is not a photo of this app"
        ))),
        None => Ok(()),
    }
}

/// A number above zero (servings).
fn positive(v: f64) -> Result<Value> {
    if v.is_finite() && v > 0.0 {
        Ok(v.into())
    } else {
        Err(Error::Invalid(format!("{v} is not above zero")))
    }
}

pub fn edit_recipe(db: &mut Db, id: i64, edit: &RecipeEdit) -> Result<Recipe> {
    if let Some(Some(sv)) = edit.servings {
        positive(sv)?;
    }
    if let Some(p) = &edit.photos {
        check_photos(p)?;
    }
    Ok(Recipe::from(&db.update(id, edit.to_patch())?))
}

pub fn set_favorite(db: &mut Db, id: i64, favorite: bool) -> Result<Recipe> {
    edit_recipe(
        db,
        id,
        &RecipeEdit {
            favorite: Some(favorite),
            ..RecipeEdit::default()
        },
    )
}

pub fn set_tags(db: &mut Db, id: i64, tags: &[String]) -> Result<()> {
    db.set_tags(id, tags).map(|_| ())
}

// ---- steps -------------------------------------------------------------------------------------

fn expect(db: &Db, id: i64, kind: &str) -> Result<()> {
    let found = db.get(id)?.kind;
    if found == kind {
        Ok(())
    } else {
        Err(Error::Invalid(format!(
            "item {id} is a {found}, not a {kind}"
        )))
    }
}

/// Add a step at the end of a recipe.
pub fn add_step(db: &mut Db, recipe: i64, text: &str) -> Result<Step> {
    expect(db, recipe, RECIPE)?;
    let mut new = NewItem::new(STEP, &title_of(text)).parent(recipe);
    if !text.is_empty() {
        new = new.field("text", text);
    }
    Ok(Step::from(&db.create(new)?))
}

/// Change a step's text or photos. Its title follows its text.
pub fn edit_step(db: &mut Db, id: i64, edit: &StepEdit) -> Result<Step> {
    expect(db, id, STEP)?;
    let mut patch = ItemPatch::default();
    if let Some(t) = &edit.text {
        patch = patch.title(&title_of(t)).field(
            "text",
            if t.is_empty() {
                Value::Null
            } else {
                t.clone().into()
            },
        );
    }
    if let Some(p) = &edit.photos {
        check_photos(p)?;
        patch = patch.field(
            "photos",
            if p.is_empty() {
                Value::Null
            } else {
                p.clone().into()
            },
        );
    }
    Ok(Step::from(&db.update(id, patch)?))
}

/// Move a step `step` places (-1 earlier, 1 later) among its recipe's steps. At an end, nothing
/// changes.
pub fn move_step(db: &mut Db, id: i64, step: i32) -> Result<()> {
    expect(db, id, STEP)?;
    move_among(db, id, step, STEP)
}

/// Move a child `step` places among its siblings of the same kind (a recipe also has ingredients).
fn move_among(db: &mut Db, id: i64, step: i32, kind: &str) -> Result<()> {
    let parent = db.get(id)?.parent_id;
    let siblings: Vec<_> = db
        .children(parent)?
        .into_iter()
        .filter(|s| s.kind == kind)
        .collect();
    let Some(at) = siblings.iter().position(|s| s.id == id) else {
        return Err(Error::NotFound(id));
    };
    let Some(to) = usize::try_from(at as i32 + step)
        .ok()
        .filter(|i| *i < siblings.len())
    else {
        return Ok(());
    };
    // `move_item` inserts before the item at a position: before the neighbour to go earlier, just
    // after it to go later.
    let neighbour = siblings[to].position;
    db.move_item(
        id,
        parent,
        Some(if step < 0 { neighbour } else { neighbour + 1 }),
    )?;
    Ok(())
}

pub fn delete_step(db: &mut Db, id: i64) -> Result<()> {
    expect(db, id, STEP)?;
    db.delete(id)?;
    Ok(())
}

// ---- ingredients -------------------------------------------------------------------------------

fn number(q: Option<f64>) -> Result<Value> {
    match q {
        None => Ok(Value::Null),
        Some(q) if q.is_finite() && q >= 0.0 => Ok(q.into()),
        Some(q) => Err(Error::Invalid(format!("{q} is not a quantity"))),
    }
}

fn named(name: &str) -> Result<&str> {
    let n = name.trim();
    if n.is_empty() {
        Err(Error::Invalid("an ingredient needs a name".into()))
    } else {
        Ok(n)
    }
}

/// Add an ingredient at the end of a recipe. No quantity means "to taste": it only has to be there.
pub fn add_ingredient(
    db: &mut Db,
    recipe: i64,
    name: &str,
    quantity: Option<f64>,
    unit: &str,
    optional: bool,
) -> Result<Ingredient> {
    expect(db, recipe, RECIPE)?;
    let mut new = NewItem::new(INGREDIENT, named(name)?).parent(recipe);
    if let Some(q) = quantity {
        new = new.field("quantity", number(Some(q))?);
    }
    if !unit.trim().is_empty() {
        new = new.field("unit", unit.trim());
    }
    if optional {
        new = new.field("optional", true);
    }
    Ok(Ingredient::from(&db.create(new)?))
}

pub fn edit_ingredient(db: &mut Db, id: i64, edit: &IngredientEdit) -> Result<Ingredient> {
    expect(db, id, INGREDIENT)?;
    let mut patch = ItemPatch::default();
    if let Some(n) = &edit.name {
        patch = patch.title(named(n)?);
    }
    if let Some(q) = edit.quantity {
        patch = patch.field("quantity", number(q)?);
    }
    if let Some(u) = &edit.unit {
        let u = u.trim();
        patch = patch.field("unit", if u.is_empty() { Value::Null } else { u.into() });
    }
    if let Some(o) = edit.optional {
        patch = patch.field("optional", o);
    }
    Ok(Ingredient::from(&db.update(id, patch)?))
}

pub fn move_ingredient(db: &mut Db, id: i64, step: i32) -> Result<()> {
    expect(db, id, INGREDIENT)?;
    move_among(db, id, step, INGREDIENT)
}

pub fn delete_ingredient(db: &mut Db, id: i64) -> Result<()> {
    expect(db, id, INGREDIENT)?;
    db.delete(id)?;
    Ok(())
}

// ---- the fridge --------------------------------------------------------------------------------

/// Put something in the fridge. Adding what is already there (same name, unit and expiry) adds to
/// it.
pub fn add_stock(
    db: &mut Db,
    name: &str,
    quantity: Option<f64>,
    unit: &str,
    expires: Option<NaiveDate>,
) -> Result<Stock> {
    let name = named(name)?;
    let key = crate::shopping::normalize(name);
    let unit = unit.trim();
    let same = db
        .list(&organiser_engine::Query {
            kind: Some(STOCK.into()),
            ..organiser_engine::Query::default()
        })?
        .iter()
        .map(Stock::from)
        .find(|s| {
            crate::shopping::normalize(&s.name) == key
                && s.unit.eq_ignore_ascii_case(unit)
                && s.expires == expires
        });
    if let Some(have) = same {
        let total = match (have.quantity, quantity) {
            (Some(a), Some(b)) => Some(a + b),
            (a, b) => a.or(b),
        };
        return edit_stock(db, have.id, Some(total), None, None);
    }
    let mut new = NewItem::new(STOCK, name);
    if let Some(q) = quantity {
        new = new.field("quantity", number(Some(q))?);
    }
    if !unit.is_empty() {
        new = new.field("unit", unit);
    }
    if let Some(e) = expires {
        new = new.field("expires", e.to_string());
    }
    Ok(Stock::from(&db.create(new)?))
}

/// Change a fridge item's quantity (`Some(None)` clears it: "some"), unit or expiry (`Some(None)`
/// clears it).
pub fn edit_stock(
    db: &mut Db,
    id: i64,
    quantity: Option<Option<f64>>,
    unit: Option<&str>,
    expires: Option<Option<NaiveDate>>,
) -> Result<Stock> {
    expect(db, id, STOCK)?;
    let mut patch = ItemPatch::default();
    if let Some(q) = quantity {
        patch = patch.field("quantity", number(q)?);
    }
    if let Some(u) = unit {
        let u = u.trim();
        patch = patch.field("unit", if u.is_empty() { Value::Null } else { u.into() });
    }
    if let Some(e) = expires {
        patch = patch.field("expires", e.map_or(Value::Null, |d| d.to_string().into()));
    }
    Ok(Stock::from(&db.update(id, patch)?))
}

/// Rename a fridge item. The name cannot be empty.
pub fn rename_stock(db: &mut Db, id: i64, name: &str) -> Result<Stock> {
    expect(db, id, STOCK)?;
    Ok(Stock::from(
        &db.update(id, ItemPatch::default().title(named(name)?))?,
    ))
}

/// Move a fridge item `step` places (negative earlier) in the saved order. At an end, nothing
/// changes.
pub fn move_stock(db: &mut Db, id: i64, step: i32) -> Result<()> {
    expect(db, id, STOCK)?;
    move_among(db, id, step, STOCK)
}

pub fn delete_stock(db: &mut Db, id: i64) -> Result<()> {
    expect(db, id, STOCK)?;
    db.delete(id)?;
    Ok(())
}

// ---- meals: the calendar -----------------------------------------------------------------------

/// Put a recipe on a day, at a time of day if you say, for `servings` (at least 1). A day up to
/// today means it was cooked, a later day that it is planned. The meal keeps the recipe's title.
pub fn add_meal(
    db: &mut Db,
    recipe: i64,
    date: NaiveDate,
    time: Option<MealTime>,
    servings: f64,
    note: &str,
) -> Result<Meal> {
    let item = db.get(recipe)?;
    if item.kind != RECIPE {
        return Err(Error::Invalid(format!(
            "item {recipe} is a {}, not a recipe",
            item.kind
        )));
    }
    check_servings(servings)?;
    let mut new = NewItem::new(MEAL, &item.title)
        .field("recipe", recipe)
        .field("date", date.to_string())
        .field("servings", servings);
    if let Some(t) = time {
        new = new.field("time", t.as_str());
    }
    if !note.is_empty() {
        new = new.field("note", note);
    }
    Ok(Meal::from(&db.create(new)?))
}

fn check_servings(servings: f64) -> Result<()> {
    if servings.is_finite() && servings >= 1.0 {
        Ok(())
    } else {
        Err(Error::Invalid(format!(
            "{servings} servings is not at least 1"
        )))
    }
}

/// Change a meal. `time: Some(None)` clears the time of day.
pub fn edit_meal(
    db: &mut Db,
    id: i64,
    date: Option<NaiveDate>,
    time: Option<Option<MealTime>>,
    servings: Option<f64>,
    note: Option<&str>,
) -> Result<Meal> {
    expect(db, id, MEAL)?;
    let mut patch = ItemPatch::default();
    if let Some(d) = date {
        patch = patch.field("date", d.to_string());
    }
    if let Some(t) = time {
        patch = patch.field("time", t.map_or(Value::Null, |t| t.as_str().into()));
    }
    if let Some(s) = servings {
        check_servings(s)?;
        patch = patch.field("servings", s);
    }
    if let Some(n) = note {
        patch = patch.field("note", if n.is_empty() { Value::Null } else { n.into() });
    }
    Ok(Meal::from(&db.update(id, patch)?))
}

pub fn delete_meal(db: &mut Db, id: i64) -> Result<()> {
    expect(db, id, MEAL)?;
    db.delete(id)?;
    Ok(())
}

// ---- the shopping list ------------------------------------------------------------------------

/// Remember the recipes picked for the shopping list and their portions. The list is one item; its
/// last-changed time moves only when the picks change.
pub fn save_shopping(db: &mut Db, picks: &[(i64, f64)]) -> Result<ShopList> {
    let value: Vec<String> = picks.iter().map(|(r, p)| format!("{r}:{p}")).collect();
    let found = db.list(&organiser_engine::Query {
        kind: Some(SHOPLIST.into()),
        ..organiser_engine::Query::default()
    })?;
    let item = match found.into_iter().next() {
        Some(i) => {
            if ShopList::from_item(&i).picks == picks {
                return Ok(ShopList::from_item(&i));
            }
            db.update(
                i.id,
                ItemPatch::default().field(
                    "picks",
                    if value.is_empty() {
                        Value::Null
                    } else {
                        value.into()
                    },
                ),
            )?
        }
        None => {
            let mut new = NewItem::new(SHOPLIST, "Shopping list");
            if !value.is_empty() {
                new = new.field("picks", value);
            }
            db.create(new)?
        }
    };
    Ok(ShopList::from_item(&item))
}
