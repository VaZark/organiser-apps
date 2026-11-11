//! `recipes`: the Recipes app on the command line. Generic commands (`add list show edit rm mv tag …`)
//! come from `organiser-cli-kit`; the rest are thin calls into `recipes-schema`.
//!
//! A recipe belongs to a list: `recipes add "Soup" -p 1` adds it to list 1. Then `ing-add`, `step-add`,
//! `photo-add`; `cook` puts it on the calendar; `fridge-add` says what you have and `missing` /
//! `shopping` say what to get.

use std::process::ExitCode;

use chrono::{Datelike, Local, NaiveDate};
use clap::{Arg, ArgAction, ArgMatches, Command};
use organiser_cli_kit::{Extension, Output};
use organiser_engine::{Db, Error, Result};
use recipes_schema::lifecycle::{
    add_ingredient, add_meal, add_step, add_stock, delete_ingredient, delete_meal, delete_step,
    delete_stock, edit_ingredient, edit_meal, edit_recipe, edit_step, edit_stock, move_ingredient,
    move_step, set_favorite,
};
use recipes_schema::mealtime::MealTime;
use recipes_schema::model::{IngredientEdit, Recipe, RecipeEdit, StepEdit};
use recipes_schema::shopping::{Item, Status};
use recipes_schema::units::pretty;
use recipes_schema::views::{
    Filter, Sort, ingredients, meals_between, recipes, shopping_list, steps, stock, times_for,
};
use recipes_schema::{RECIPE, STEP, photos};
use serde_json::json;

struct Recipes;

fn id() -> Arg {
    Arg::new("id")
        .required(true)
        .value_parser(clap::value_parser!(i64))
}

fn stars(r: Option<u8>) -> String {
    r.map(|r| {
        format!(
            "{}{}",
            "★".repeat(r as usize),
            "☆".repeat(5usize.saturating_sub(r as usize))
        )
    })
    .unwrap_or_default()
}

fn entry_line(e: &Recipe) -> String {
    let mut s = format!(
        "{} {:>4}  {}",
        if e.favorite { "♥" } else { " " },
        e.id,
        e.title
    );
    if e.rating.is_some() {
        s.push_str(&format!("  {}", stars(e.rating)));
    }
    if !e.tags.is_empty() {
        s.push_str(&format!("  #{}", e.tags.join(" #")));
    }
    s
}

fn recipes_out(v: &[Recipe]) -> Output {
    Output::Custom {
        human: if v.is_empty() {
            "(none)".into()
        } else {
            v.iter().map(entry_line).collect::<Vec<_>>().join("\n")
        },
        json: json!(v),
    }
}

fn text(m: &ArgMatches, name: &str) -> String {
    m.get_one::<String>(name).cloned().unwrap_or_default()
}

fn num(m: &ArgMatches, name: &str) -> Result<Option<f64>> {
    m.get_one::<String>(name)
        .map(|v| {
            v.replace(',', ".")
                .parse::<f64>()
                .map_err(|_| Error::Invalid(format!("{name}: {v:?} is not a number")))
        })
        .transpose()
}

fn date(m: &ArgMatches, name: &str) -> Result<Option<NaiveDate>> {
    m.get_one::<String>(name)
        .map(|v| {
            v.parse::<NaiveDate>()
                .map_err(|_| Error::Invalid(format!("{name}: {v:?} is not a date (YYYY-MM-DD)")))
        })
        .transpose()
}

/// Monday to Sunday of the week holding `d`.
fn week_of(d: NaiveDate) -> (NaiveDate, NaiveDate) {
    let from = d - chrono::Days::new(u64::from(d.weekday().num_days_from_monday()));
    (from, from + chrono::Days::new(6))
}

fn amount(q: Option<f64>, unit: &str) -> String {
    match q {
        None => String::new(),
        Some(q) if unit.is_empty() => pretty(q),
        Some(q) => format!("{} {unit}", pretty(q)),
    }
}

fn shopping_out(items: Vec<Item>, all: bool) -> Output {
    let rows: Vec<&Item> = items
        .iter()
        .filter(|i| all || i.status != Status::Enough)
        .collect();
    let human = if rows.is_empty() {
        "nothing to get".to_string()
    } else {
        rows.iter()
            .map(|i| {
                let get = i
                    .get
                    .as_ref()
                    .map_or(String::new(), |(q, u)| amount(Some(*q), u));
                let note = match i.status {
                    Status::Enough => "have it",
                    Status::Missing => "",
                    Status::Short => "short",
                    Status::Unit => "have some, in another unit",
                };
                let opt = if i.optional { " (optional)" } else { "" };
                format!("{:<24} {get:<12} {note}{opt}", i.name)
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let json = rows
        .iter()
        .map(|i| {
            json!({
                "name": i.name, "status": format!("{:?}", i.status).to_lowercase(),
                "need": i.need, "get": i.get, "optional": i.optional,
            })
        })
        .collect::<Vec<_>>();
    Output::Custom {
        human,
        json: json!(json),
    }
}

/// Add a photo to a recipe or a step: copy the file into the app's photos folder and list it.
fn add_photo(db: &mut Db, id: i64, file: &str) -> Result<Output> {
    let kind = db.get(id)?.kind;
    let name = photos::import(&photos::dir(), std::path::Path::new(file))?;
    match kind.as_str() {
        RECIPE => {
            let mut all = recipes_schema::model::Recipe::from(&db.get(id)?).photos;
            all.push(name.clone());
            edit_recipe(
                db,
                id,
                &RecipeEdit {
                    photos: Some(all),
                    ..RecipeEdit::default()
                },
            )?;
        }
        STEP => {
            let mut all = recipes_schema::model::Step::from(&db.get(id)?).photos;
            all.push(name.clone());
            edit_step(
                db,
                id,
                &StepEdit {
                    photos: Some(all),
                    ..StepEdit::default()
                },
            )?;
        }
        other => {
            return Err(Error::Invalid(format!(
                "item {id} is a {other}: photos go on recipes or steps"
            )));
        }
    }
    Ok(Output::Message(format!("added {name}")))
}

fn remove_photo(db: &mut Db, id: i64, name: &str) -> Result<Output> {
    let kind = db.get(id)?.kind;
    match kind.as_str() {
        RECIPE => {
            let mut all = recipes_schema::model::Recipe::from(&db.get(id)?).photos;
            all.retain(|p| p != name);
            edit_recipe(
                db,
                id,
                &RecipeEdit {
                    photos: Some(all),
                    ..RecipeEdit::default()
                },
            )?;
        }
        STEP => {
            let mut all = recipes_schema::model::Step::from(&db.get(id)?).photos;
            all.retain(|p| p != name);
            edit_step(
                db,
                id,
                &StepEdit {
                    photos: Some(all),
                    ..StepEdit::default()
                },
            )?;
        }
        other => {
            return Err(Error::Invalid(format!(
                "item {id} is a {other}: photos go on recipes or steps"
            )));
        }
    }
    let n = photos::gc(db, &photos::dir())?;
    Ok(Output::Message(format!(
        "removed {name} ({n} file{} deleted)",
        if n == 1 { "" } else { "s" }
    )))
}

/// `shopping 3:10 5`: recipe 3 for 10 portions, recipe 5 once.
fn shopping_cmd(db: &mut Db, m: &ArgMatches) -> Result<Output> {
    let mut picks = Vec::new();
    for p in m.get_many::<String>("picks").into_iter().flatten() {
        let (id, portions) = match p.split_once(':') {
            Some((i, n)) => (i, Some(n)),
            None => (p.as_str(), None),
        };
        let id: i64 = id
            .parse()
            .map_err(|_| Error::Invalid(format!("{p:?}: expected RECIPE or RECIPE:PORTIONS")))?;
        let times = match portions {
            Some(n) => {
                let n: f64 = n
                    .replace(',', ".")
                    .parse()
                    .map_err(|_| Error::Invalid(format!("{p:?}: portions is not a number")))?;
                let servings = recipes_schema::model::Recipe::from(&db.get(id)?).servings;
                times_for(servings, n)
            }
            None => 1.0,
        };
        picks.push((id, times));
    }
    let items = shopping_list(
        db,
        &picks,
        Local::now().date_naive(),
        m.get_flag("optional"),
    )?;
    Ok(match m.get_one::<String>("format").map(String::as_str) {
        Some("md") => Output::Message(recipes_schema::views::shopping_markdown(&items)),
        Some("json") => shopping_out(items, false),
        _ => shopping_out(items, false),
    })
}

fn on_off(m: &ArgMatches, name: &str) -> Result<Option<bool>> {
    m.get_one::<String>(name)
        .map(|v| match v.as_str() {
            "on" | "true" | "yes" => Ok(true),
            "off" | "false" | "no" => Ok(false),
            other => Err(Error::Invalid(format!(
                "{name}: expected on or off, got {other:?}"
            ))),
        })
        .transpose()
}

impl Extension for Recipes {
    fn default_kind(&self) -> Option<&'static str> {
        Some(recipes_schema::RECIPE)
    }

    fn commands(&self) -> Vec<Command> {
        let toggle = |name: &'static str, help: &'static str| {
            Arg::new(name).long(name).value_name("on|off").help(help)
        };
        vec![
            Command::new("fave")
                .about("Mark an entry as a favourite")
                .arg(id()),
            Command::new("unfave")
                .about("Remove the favourite mark")
                .arg(id()),
            Command::new("steps")
                .about("A recipe's steps, in order")
                .arg(id()),
            Command::new("step-add")
                .about("Add a step at the end of a recipe (text may span lines)")
                .arg(id())
                .arg(Arg::new("text").required(true)),
            Command::new("step-set")
                .about("Change a step's text (the id is the step's)")
                .arg(id())
                .arg(Arg::new("text").required(true)),
            Command::new("step-rm").about("Delete a step").arg(id()),
            Command::new("step-mv")
                .about("Move a step up or down")
                .arg(id())
                .arg(Arg::new("way").required(true).value_parser(["up", "down"])),
            Command::new("photo-add")
                .about("Add a photo to a recipe or a step (copied into the app's photos)")
                .arg(id())
                .arg(Arg::new("file").required(true)),
            Command::new("photo-rm")
                .about("Remove a photo from a recipe or a step")
                .arg(id())
                .arg(Arg::new("name").required(true)),
            Command::new("ingredients")
                .about("What a recipe needs")
                .arg(id()),
            Command::new("ing-add")
                .about("Add an ingredient to a recipe; no --qty means to taste")
                .arg(id())
                .arg(Arg::new("name").required(true))
                .arg(Arg::new("qty").long("qty").short('q'))
                .arg(Arg::new("unit").long("unit").short('u').default_value(""))
                .arg(
                    Arg::new("optional")
                        .long("optional")
                        .action(ArgAction::SetTrue)
                        .help("never counts as missing"),
                ),
            Command::new("ing-set")
                .about("Change an ingredient (the id is the ingredient's)")
                .arg(id())
                .arg(Arg::new("name").long("name"))
                .arg(Arg::new("qty").long("qty").short('q').help("a number, or `none`"))
                .arg(Arg::new("unit").long("unit").short('u'))
                .arg(toggle("optional", "optional or required")),
            Command::new("ing-rm").about("Delete an ingredient").arg(id()),
            Command::new("ing-mv")
                .about("Move an ingredient up or down")
                .arg(id())
                .arg(Arg::new("way").required(true).value_parser(["up", "down"])),
            Command::new("fridge").about("What you have, soonest to expire first"),
            Command::new("fridge-add")
                .about("Put something in the fridge (adds to what is there)")
                .arg(Arg::new("name").required(true))
                .arg(Arg::new("qty").long("qty").short('q'))
                .arg(Arg::new("unit").long("unit").short('u').default_value(""))
                .arg(Arg::new("expires").long("expires").short('e').help("last good day, YYYY-MM-DD")),
            Command::new("fridge-set")
                .about("Correct a fridge item (--qty none for \"some\", --expires none for no date)")
                .arg(id())
                .arg(Arg::new("qty").long("qty").short('q'))
                .arg(Arg::new("unit").long("unit").short('u'))
                .arg(Arg::new("expires").long("expires").short('e'))
                .arg(Arg::new("name").long("name").short('n').help("rename it")),
            Command::new("fridge-rm").about("Remove from the fridge").arg(id()),
            Command::new("cook")
                .about("Put a recipe on the log: cooked (today or earlier) or planned (later)")
                .arg(id())
                .arg(Arg::new("date").long("date").help("YYYY-MM-DD, default today"))
                .arg(
                    Arg::new("meal")
                        .long("meal")
                        .short('m')
                        .value_parser(["morning", "brunch", "lunch", "dinner", "evening"]),
                )
                .arg(Arg::new("servings").long("servings").short('s').help("default 1"))
                .arg(Arg::new("note").long("note").default_value("")),
            Command::new("calendar")
                .about("Meals in a range (default: this week)")
                .arg(Arg::new("from").long("from"))
                .arg(Arg::new("to").long("to")),
            Command::new("meal-set")
                .about("Change a meal (the id is the meal's)")
                .arg(id())
                .arg(Arg::new("date").long("date"))
                .arg(
                    Arg::new("meal")
                        .long("meal")
                        .short('m')
                        .value_parser(["morning", "brunch", "lunch", "dinner", "evening", "none"]),
                )
                .arg(Arg::new("servings").long("servings").short('s'))
                .arg(Arg::new("note").long("note")),
            Command::new("meal-rm").about("Remove a meal").arg(id()),
            Command::new("shopping")
                .about("Prepare a shopping list from recipes and the fridge (expired food does not count)")
                .arg(
                    Arg::new("picks")
                        .required(true)
                        .num_args(1..)
                        .help("RECIPE or RECIPE:PORTIONS, e.g. 3:10 (10 portions of recipe 3)"),
                )
                .arg(
                    Arg::new("format")
                        .long("format")
                        .short('f')
                        .value_parser(["list", "md", "json"])
                        .default_value("list")
                        .help("md: task lines to paste into a Todo note"),
                )
                .arg(Arg::new("optional").long("optional").action(ArgAction::SetTrue).help("include optional ingredients")),
            Command::new("favorites")
                .about("Favourites")
                .arg(
                    Arg::new("sort")
                        .long("sort")
                        .value_parser(["manual", "title", "rating", "recent"])
                        .default_value("rating"),
                ),
        ]
    }

    fn run(&self, db: &mut Db, name: &str, m: &ArgMatches) -> Option<Result<Output>> {
        let id = || *m.get_one::<i64>("id").unwrap_or(&0);
        Some(match name {
            "fave" => set_favorite(db, id(), true).map(|e| recipes_out(&[e])),
            "unfave" => set_favorite(db, id(), false).map(|e| recipes_out(&[e])),
            "steps" => steps(db, id()).map(|v| Output::Custom {
                human: if v.is_empty() {
                    "(none)".into()
                } else {
                    v.iter()
                        .enumerate()
                        .map(|(n, s)| {
                            let photos = if s.photos.is_empty() {
                                String::new()
                            } else {
                                format!(
                                    "  [{} photo{}]",
                                    s.photos.len(),
                                    if s.photos.len() == 1 { "" } else { "s" }
                                )
                            };
                            format!(
                                "{:>2}. ({}) {}{photos}",
                                n + 1,
                                s.id,
                                s.text.replace('\n', "\n      ")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                },
                json: json!(v),
            }),
            "step-add" => add_step(db, id(), &text(m, "text"))
                .map(|s| Output::Message(format!("added step {}", s.id))),
            "step-set" => edit_step(
                db,
                id(),
                &StepEdit {
                    text: Some(text(m, "text")),
                    photos: None,
                },
            )
            .map(|s| Output::Message(format!("step {} changed", s.id))),
            "step-rm" => delete_step(db, id()).map(|()| Output::Message("deleted".into())),
            "step-mv" => move_step(db, id(), if text(m, "way") == "up" { -1 } else { 1 })
                .map(|()| Output::Message("moved".into())),
            "photo-add" => add_photo(db, id(), &text(m, "file")),
            "photo-rm" => remove_photo(db, id(), &text(m, "name")),
            "ingredients" => ingredients(db, id()).map(|v| Output::Custom {
                human: if v.is_empty() {
                    "(none)".into()
                } else {
                    v.iter()
                        .map(|i| {
                            format!(
                                "{:>4}  {:<10} {}{}",
                                i.id,
                                amount(i.quantity, &i.unit),
                                i.name,
                                if i.optional { "  (optional)" } else { "" }
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                },
                json: json!(v),
            }),
            "ing-add" => num(m, "qty").and_then(|q| {
                add_ingredient(
                    db,
                    id(),
                    &text(m, "name"),
                    q,
                    &text(m, "unit"),
                    m.get_flag("optional"),
                )
                .map(|i| Output::Message(format!("added ingredient {} ({})", i.id, i.name)))
            }),
            "ing-set" => (|| {
                let quantity = match m.get_one::<String>("qty").map(String::as_str) {
                    Some("none") => Some(None),
                    Some(_) => num(m, "qty")?.map(Some),
                    None => None,
                };
                edit_ingredient(
                    db,
                    id(),
                    &IngredientEdit {
                        name: m.get_one::<String>("name").cloned(),
                        quantity,
                        unit: m.get_one::<String>("unit").cloned(),
                        optional: on_off(m, "optional")?,
                    },
                )
                .map(|i| Output::Message(format!("{}: {}", i.name, amount(i.quantity, &i.unit))))
            })(),
            "ing-rm" => delete_ingredient(db, id()).map(|()| Output::Message("deleted".into())),
            "ing-mv" => move_ingredient(db, id(), if text(m, "way") == "up" { -1 } else { 1 })
                .map(|()| Output::Message("moved".into())),
            "fridge" => stock(db).map(|v| Output::Custom {
                human: if v.is_empty() {
                    "(empty)".into()
                } else {
                    v.iter()
                        .map(|s| {
                            let a = amount(s.quantity, &s.unit);
                            format!(
                                "{:>4}  {:<10} {}{}",
                                s.id,
                                if a.is_empty() { "some".into() } else { a },
                                s.name,
                                s.expires
                                    .map(|e| format!("  (until {e})"))
                                    .unwrap_or_default()
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                },
                json: json!(v),
            }),
            "fridge-add" => num(m, "qty").and_then(|q| {
                add_stock(
                    db,
                    &text(m, "name"),
                    q,
                    &text(m, "unit"),
                    date(m, "expires")?,
                )
                .map(|s| Output::Message(format!("{}: {}", s.name, amount(s.quantity, &s.unit))))
            }),
            "fridge-set" => (|| {
                let quantity = match m.get_one::<String>("qty").map(String::as_str) {
                    Some("none") => Some(None),
                    Some(_) => num(m, "qty")?.map(Some),
                    None => None,
                };
                let expires = match m.get_one::<String>("expires").map(String::as_str) {
                    Some("none") => Some(None),
                    Some(_) => date(m, "expires")?.map(Some),
                    None => None,
                };
                if let Some(n) = m.get_one::<String>("name") {
                    recipes_schema::lifecycle::rename_stock(db, id(), n)?;
                }
                edit_stock(
                    db,
                    id(),
                    quantity,
                    m.get_one::<String>("unit").map(String::as_str),
                    expires,
                )
                .map(|s| Output::Message(format!("{}: {}", s.name, amount(s.quantity, &s.unit))))
            })(),
            "fridge-rm" => delete_stock(db, id()).map(|()| Output::Message("removed".into())),
            "cook" => (|| {
                let day = date(m, "date")?.unwrap_or_else(|| Local::now().date_naive());
                let servings = num(m, "servings")?.unwrap_or(1.0);
                let time = m.get_one::<String>("meal").and_then(|t| MealTime::parse(t));
                add_meal(db, id(), day, time, servings, &text(m, "note")).map(|c| {
                    let what = if c.is_planned(Local::now().date_naive()) {
                        "planned"
                    } else {
                        "cooked"
                    };
                    Output::Message(format!("{what} {} on {} (meal {})", c.title, c.date, c.id))
                })
            })(),
            "calendar" => (|| {
                let (wf, wt) = week_of(Local::now().date_naive());
                let v = meals_between(
                    db,
                    date(m, "from")?.unwrap_or(wf),
                    date(m, "to")?.unwrap_or(wt),
                )?;
                let today = Local::now().date_naive();
                Ok(Output::Custom {
                    human: if v.is_empty() {
                        "(nothing)".into()
                    } else {
                        v.iter()
                            .map(|c| {
                                let when = c
                                    .time
                                    .map(|t| format!(" {:<8}", t.as_str()))
                                    .unwrap_or_else(|| " ".repeat(9));
                                let x = if c.servings > 1.0 {
                                    format!("  x{}", pretty(c.servings))
                                } else {
                                    String::new()
                                };
                                let note = if c.note.is_empty() {
                                    String::new()
                                } else {
                                    format!("  — {}", c.note)
                                };
                                format!(
                                    "{}{when} {:>4}  {}{x}{note}{}",
                                    c.date,
                                    c.id,
                                    c.title,
                                    if c.is_planned(today) {
                                        "  (planned)"
                                    } else {
                                        ""
                                    }
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    },
                    json: json!(v),
                })
            })(),
            "meal-set" => (|| {
                let time = match m.get_one::<String>("meal").map(String::as_str) {
                    Some("none") => Some(None),
                    Some(t) => Some(MealTime::parse(t)),
                    None => None,
                };
                edit_meal(
                    db,
                    id(),
                    date(m, "date")?,
                    time,
                    num(m, "servings")?,
                    m.get_one::<String>("note").map(String::as_str),
                )
                .map(|c| {
                    Output::Message(format!("{} on {} x{}", c.title, c.date, pretty(c.servings)))
                })
            })(),
            "meal-rm" => delete_meal(db, id()).map(|()| Output::Message("removed".into())),
            "shopping" => shopping_cmd(db, m),
            "favorites" => {
                let sort = match m.get_one::<String>("sort").map(String::as_str) {
                    Some("title") => Sort::Title,
                    Some("recent") => Sort::Recent,
                    Some("manual") => Sort::Manual,
                    _ => Sort::Rating,
                };
                recipes(
                    db,
                    &Filter {
                        favorites: true,
                        sort,
                        ..Filter::default()
                    },
                )
                .map(|v| recipes_out(&v))
            }
            _ => return None,
        })
    }
}

fn main() -> ExitCode {
    organiser_cli_kit::run(&recipes_schema::META, &recipes_schema::SCHEMA, &Recipes)
}
