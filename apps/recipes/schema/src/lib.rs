//! # recipes-schema
//!
//! The Recipes app: **recipes**, organised with tags. A recipe has **ingredients** (a name, a
//! quantity and a unit each), **steps** in order (each its own multiline text with photos), photos
//! of its own, how many it serves and a personal note; its ingredients and steps belong to it, so
//! deleting a recipe deletes both.
//!
//! Two more parts sit beside the recipes:
//!
//! * the **fridge**: what you have, as items with a quantity and an optional expiry date ([`STOCK`]);
//! * the **calendar**: **meals**, a recipe on a date, at a time of day, for some servings ([`MEAL`]). A date up to today
//!   is cooked, a later one is planned. A meal keeps the recipe's title, so it outlives the recipe.
//!
//! Recipes and the fridge are separate: a **shopping list** is prepared on request from recipes you
//! pick (portions each) and what is in the fridge and not expired ([`shopping`]).
//!
//! Layers, as in the engine (`tests/layers.rs` enforces them): [`primitives`] (the pure model) and
//! [`components`] (operations on a [`organiser_engine::Db`]).

pub mod components;
pub mod primitives;

use organiser_core::AppMeta;
use organiser_engine::{Field, FieldType, Kind, Schema};

pub use components::{lifecycle, photos, views};
pub use primitives::{calendar, mealtime, model, photo, shopping, step, units};

pub const META: AppMeta = AppMeta {
    id: "io.github.vazark.Recipes",
    slug: "recipes",
    name: "Recipes",
    comment: "Keep your recipes, step by step, and what you cooked",
    categories: &["Utility"],
};

pub const RECIPE: &str = "recipe";
pub const STEP: &str = "step";
pub const INGREDIENT: &str = "ingredient";
pub const STOCK: &str = "stock";
pub const MEAL: &str = "meal";
pub const SHOPLIST: &str = "shoplist";

pub static SCHEMA: Schema = Schema {
    kinds: &[
        Kind {
            name: RECIPE,
            fields: &[
                // The person's own note on the whole recipe.
                Field::new("notes", FieldType::Markdown).searchable(),
                // File names in the photos folder (see `photos`).
                Field::new("photos", FieldType::List),
                // How many portions the recipe makes, for scaling a shopping list.
                Field::new("servings", FieldType::Number),
                Field::new("favorite", FieldType::Bool).indexed(),
                Field::new("rating", FieldType::Rating).indexed(),
            ],
        },
        // A step of a recipe, in order (the engine keeps the position).
        Kind {
            name: STEP,
            fields: &[
                Field::new("text", FieldType::Markdown).searchable(),
                Field::new("photos", FieldType::List),
            ],
        },
        // What a recipe needs. The title is the ingredient's name.
        Kind {
            name: INGREDIENT,
            fields: &[
                // Absent: "to taste", "a pinch": only needs to be there.
                Field::new("quantity", FieldType::Number),
                Field::new("unit", FieldType::Text),
                // Left out of what is missing, and of whether a recipe can be made.
                Field::new("optional", FieldType::Bool),
            ],
        },
        // What is in the fridge. The title is the ingredient's name.
        Kind {
            name: STOCK,
            fields: &[
                Field::new("quantity", FieldType::Number),
                Field::new("unit", FieldType::Text),
                // Optional: past it, the item no longer counts as something you have.
                Field::new("expires", FieldType::Date).indexed(),
            ],
        },
        // The shopping list being prepared: one item, remembered with the day it was last changed.
        // The picks are `recipe id:portions` strings.
        Kind {
            name: SHOPLIST,
            fields: &[Field::new("picks", FieldType::List)],
        },
        // A recipe on a day: cooked (up to today) or planned (later), this many times over.
        Kind {
            name: MEAL,
            fields: &[
                Field::new("recipe", FieldType::Reference(RECIPE)).indexed(),
                Field::new("date", FieldType::Date).indexed().required(),
                // When in the day: morning, brunch, lunch, dinner, evening (optional).
                Field::new("time", FieldType::Enum(mealtime::NAMES)).indexed(),
                // How many servings were made (meal prep for five is 5).
                Field::new("servings", FieldType::Number),
                Field::new("note", FieldType::Markdown).searchable(),
            ],
        },
    ],
};
