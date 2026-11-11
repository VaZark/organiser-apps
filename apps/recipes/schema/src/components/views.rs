//! What a frontend shows.

use organiser_engine::{Db, FieldFilter, Op, Query, Result, Sort as Order, TagFilter};

use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;

use crate::primitives::model::{Ingredient, Meal, Recipe, ShopList, Step, Stock};
use crate::primitives::shopping::{self, Item};
use crate::{INGREDIENT, MEAL, RECIPE, SHOPLIST, STEP, STOCK};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Sort {
    /// The order the person arranged.
    #[default]
    Manual,
    Title,
    /// Best rated first; unrated last.
    Rating,
    /// Newest first.
    Recent,
}

#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub text: Option<String>,
    pub tags: TagFilter,
    pub favorites: bool,
    /// At least this many stars. Unrated recipes do not match.
    pub min_rating: Option<u8>,
    /// What the person chose in the filters the schema offers (favourite, rating).
    pub fields: organiser_engine::Filters,
    pub sort: Sort,
}

/// The ids of the recipes whose own text or whose steps match `text`.
fn matching(db: &Db, text: &str) -> Result<HashSet<i64>> {
    let mut ids: HashSet<i64> = db
        .list(&Query {
            kind: Some(RECIPE.into()),
            text: Some(text.into()),
            ..Query::default()
        })?
        .iter()
        .map(|i| i.id)
        .collect();
    // A recipe is found by what its steps say, and by its ingredients' names, too.
    for kind in [STEP, INGREDIENT] {
        ids.extend(
            db.list(&Query {
                kind: Some(kind.into()),
                text: Some(text.into()),
                ..Query::default()
            })?
            .iter()
            .filter_map(|s| s.parent_id),
        );
    }
    Ok(ids)
}

pub fn recipes(db: &Db, f: &Filter) -> Result<Vec<Recipe>> {
    let text = f.text.as_deref().map(str::trim).filter(|t| !t.is_empty());
    let found = match text {
        Some(t) => Some(matching(db, t)?),
        None => None,
    };
    let mut q = Query {
        kind: Some(RECIPE.into()),
        tags: f.tags.clone(),
        sort: match f.sort {
            Sort::Manual => vec![Order::asc("position")],
            Sort::Title => vec![Order::asc("title")],
            Sort::Rating => vec![Order::desc("rating"), Order::asc("title")],
            Sort::Recent => vec![Order::desc("created"), Order::desc("id")],
        },
        ..Query::default()
    };
    if f.favorites {
        q.filters.push(FieldFilter::new("favorite", Op::Eq, true));
    }
    if let Some(n) = f.min_rating {
        q.filters.push(FieldFilter::new("rating", Op::Ge, n));
    }
    Ok(db
        .list(&q)?
        .iter()
        .filter(|i| f.fields.matches(i))
        .filter(|i| found.as_ref().is_none_or(|ids| ids.contains(&i.id)))
        .map(Recipe::from)
        .collect())
}

/// A recipe's steps, in order.
pub fn steps(db: &Db, recipe: i64) -> Result<Vec<Step>> {
    Ok(db
        .children(Some(recipe))?
        .iter()
        .filter(|i| i.kind == STEP)
        .map(Step::from)
        .collect())
}

/// How many steps each recipe has (those with none are left out).
pub fn step_counts(db: &Db) -> Result<HashMap<i64, usize>> {
    let mut out: HashMap<i64, usize> = HashMap::new();
    for s in db.list(&Query {
        kind: Some(STEP.into()),
        ..Query::default()
    })? {
        if let Some(p) = s.parent_id {
            *out.entry(p).or_default() += 1;
        }
    }
    Ok(out)
}

/// A recipe's ingredients, in order.
pub fn ingredients(db: &Db, recipe: i64) -> Result<Vec<Ingredient>> {
    Ok(db
        .children(Some(recipe))?
        .iter()
        .filter(|i| i.kind == INGREDIENT)
        .map(Ingredient::from)
        .collect())
}

/// Everything in the fridge in the order it was saved: added at the end, and moved where the
/// person drags it.
pub fn stock(db: &Db) -> Result<Vec<Stock>> {
    Ok(db
        .list(&Query {
            kind: Some(STOCK.into()),
            sort: vec![Order::asc("position"), Order::asc("id")],
            ..Query::default()
        })?
        .iter()
        .map(Stock::from)
        .collect())
}

/// Meals between two dates, inclusive, oldest first.
pub fn meals_between(db: &Db, from: NaiveDate, to: NaiveDate) -> Result<Vec<Meal>> {
    Ok(db
        .list(&Query {
            kind: Some(MEAL.into()),
            filters: vec![
                FieldFilter::new("date", Op::Ge, from.to_string()),
                FieldFilter::new("date", Op::Le, to.to_string()),
            ],
            sort: vec![Order::asc("date"), Order::asc("id")],
            ..Query::default()
        })?
        .iter()
        .map(Meal::from)
        .collect())
}

/// Every time one recipe was put on the calendar, newest first.
pub fn meals_for(db: &Db, recipe: i64) -> Result<Vec<Meal>> {
    Ok(db
        .list(&Query {
            kind: Some(MEAL.into()),
            filters: vec![FieldFilter::new("recipe", Op::Eq, recipe)],
            sort: vec![Order::desc("date"), Order::desc("id")],
            ..Query::default()
        })?
        .iter()
        .map(Meal::from)
        .collect())
}

/// The shopping list for the recipes picked, each with how many times over it is made: every
/// ingredient across them adds up, then what the fridge has (and has not expired on `today`) is
/// taken off. Optional ingredients are left out unless `include_optional`. A recipe that is gone
/// counts for nothing.
pub fn shopping_list(
    db: &Db,
    picks: &[(i64, f64)],
    today: NaiveDate,
    include_optional: bool,
) -> Result<Vec<Item>> {
    let mut wanted = Vec::new();
    for (recipe, times) in picks {
        if db.get(*recipe).is_ok() {
            wanted.extend(shopping::needs(&ingredients(db, *recipe)?, *times));
        }
    }
    let fridge = shopping::haves(&stock(db)?, today);
    Ok(shopping::compare(&wanted, &fridge, include_optional))
}

/// A shopping list as Markdown task lines, for pasting into a note or task: `- [ ] 150 g pasta`.
pub fn shopping_markdown(items: &[Item]) -> String {
    items
        .iter()
        .filter(|i| i.status != shopping::Status::Enough)
        .map(|i| {
            let get = i
                .get
                .as_ref()
                .map(|(q, u)| {
                    if u.is_empty() {
                        crate::units::pretty(*q)
                    } else {
                        format!("{} {u}", crate::units::pretty(*q))
                    }
                })
                .unwrap_or_default();
            if get.is_empty() {
                format!("- [ ] {}", i.name)
            } else {
                format!("- [ ] {get} {}", i.name)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// How many times over a recipe is made to get `portions` portions: a recipe that makes 4 and 10
/// portions wanted is 2.5 times. A recipe with no servings counts a portion as one go.
pub fn times_for(servings: Option<f64>, portions: f64) -> f64 {
    portions / servings.filter(|s| *s > 0.0).unwrap_or(1.0)
}

/// The remembered shopping list: its picks and when they last changed (nothing if never saved).
pub fn shopping_picks(db: &Db) -> Result<ShopList> {
    Ok(db
        .list(&Query {
            kind: Some(SHOPLIST.into()),
            ..Query::default()
        })?
        .first()
        .map(ShopList::from_item)
        .unwrap_or_default())
}
