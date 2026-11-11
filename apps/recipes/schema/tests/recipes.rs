//! What makes Recipes more than a list: ordered steps, photos, finding by step text, and the
//! cooking log.

use chrono::NaiveDate;
use organiser_engine::Db;
use recipes_schema::SCHEMA;
use recipes_schema::lifecycle::{
    NewRecipe, add_ingredient, add_meal, add_recipe, add_step, add_stock, delete_ingredient,
    delete_meal, delete_step, edit_ingredient, edit_meal, edit_recipe, edit_step, edit_stock,
    move_ingredient, move_step, move_stock, rename_stock, save_shopping, set_favorite,
};
use recipes_schema::mealtime::MealTime;
use recipes_schema::model::{IngredientEdit, RecipeEdit, StepEdit};
use recipes_schema::photos;
use recipes_schema::shopping::Status;

fn day(s: &str) -> NaiveDate {
    s.parse().unwrap()
}
use recipes_schema::views::{
    Filter, ingredients, meals_between, meals_for, recipes, shopping_list, shopping_markdown,
    shopping_picks, step_counts, steps, stock,
};

fn db() -> Db {
    Db::open_in_memory(&SCHEMA).unwrap()
}

fn d(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

fn recipe(db: &mut Db, title: &str) -> i64 {
    add_recipe(db, &NewRecipe::new(title)).unwrap().id
}

fn texts(db: &Db, recipe: i64) -> Vec<String> {
    steps(db, recipe)
        .unwrap()
        .into_iter()
        .map(|s| s.text)
        .collect()
}

const PHOTO: &str = "0123456789abcdef.jpg";

// ---- steps -----------------------------------------------------------------------------------

#[test]
fn steps_keep_the_order_they_were_added_in_with_their_own_multiline_text() {
    let mut db = db();
    let r = recipe(&mut db, "Soup");
    add_step(&mut db, r, "Chop the onions").unwrap();
    add_step(&mut db, r, "Fry them\n\nuntil golden").unwrap();
    add_step(&mut db, r, "Add stock").unwrap();
    assert_eq!(
        texts(&db, r),
        ["Chop the onions", "Fry them\n\nuntil golden", "Add stock"]
    );
}

#[test]
fn a_step_can_be_moved_earlier_or_later_and_stays_put_at_the_ends() {
    let mut db = db();
    let r = recipe(&mut db, "Soup");
    let a = add_step(&mut db, r, "a").unwrap();
    let b = add_step(&mut db, r, "b").unwrap();
    let c = add_step(&mut db, r, "c").unwrap();
    move_step(&mut db, c.id, -1).unwrap();
    assert_eq!(texts(&db, r), ["a", "c", "b"]);
    move_step(&mut db, a.id, 1).unwrap();
    assert_eq!(texts(&db, r), ["c", "a", "b"]);
    move_step(&mut db, c.id, -1).unwrap();
    move_step(&mut db, b.id, 1).unwrap();
    assert_eq!(texts(&db, r), ["c", "a", "b"], "nothing moves past an end");
}

#[test]
fn steps_of_two_recipes_are_ordered_apart() {
    let mut db = db();
    let r1 = add_recipe(&mut db, &NewRecipe::new("one")).unwrap().id;
    let r2 = add_recipe(&mut db, &NewRecipe::new("two")).unwrap().id;
    add_step(&mut db, r1, "1a").unwrap();
    let b = add_step(&mut db, r2, "2a").unwrap();
    add_step(&mut db, r1, "1b").unwrap();
    add_step(&mut db, r2, "2b").unwrap();
    move_step(&mut db, b.id, 1).unwrap();
    assert_eq!(texts(&db, r1), ["1a", "1b"]);
    assert_eq!(texts(&db, r2), ["2b", "2a"]);
    assert_eq!(step_counts(&db).unwrap()[&r1], 2);
}

#[test]
fn editing_a_step_changes_its_text_and_title_and_clearing_it_leaves_an_empty_step() {
    let mut db = db();
    let r = recipe(&mut db, "Soup");
    let s = add_step(&mut db, r, "old").unwrap();
    let s = edit_step(
        &mut db,
        s.id,
        &StepEdit {
            text: Some("New text\nmore".into()),
            ..StepEdit::default()
        },
    )
    .unwrap();
    assert_eq!(s.text, "New text\nmore");
    assert_eq!(db.get(s.id).unwrap().title, "New text");
    let s = edit_step(
        &mut db,
        s.id,
        &StepEdit {
            text: Some(String::new()),
            ..StepEdit::default()
        },
    )
    .unwrap();
    assert_eq!(
        (s.text.as_str(), db.get(s.id).unwrap().title.as_str()),
        ("", "Step")
    );
}

#[test]
fn only_recipes_take_steps_and_only_steps_are_edited_as_steps() {
    let mut db = db();
    let r = recipe(&mut db, "Soup");
    let fridge = add_stock(&mut db, "milk", None, "", None).unwrap().id;
    assert!(
        add_step(&mut db, fridge, "x").is_err(),
        "only recipes have steps"
    );
    assert!(
        edit_step(&mut db, r, &StepEdit::default()).is_err(),
        "a recipe is not a step"
    );
    assert!(delete_step(&mut db, r).is_err());
}

#[test]
fn deleting_a_recipe_deletes_its_steps_and_a_step_can_be_deleted() {
    let mut db = db();
    let r = recipe(&mut db, "Soup");
    let a = add_step(&mut db, r, "a").unwrap();
    add_step(&mut db, r, "b").unwrap();
    delete_step(&mut db, a.id).unwrap();
    assert_eq!(texts(&db, r), ["b"]);
    assert_eq!(db.delete(r).unwrap(), 2, "the recipe and its step");
    assert!(step_counts(&db).unwrap().is_empty());
}

// ---- photos ----------------------------------------------------------------------------------

#[test]
fn a_recipe_and_a_step_each_keep_their_own_photos() {
    let mut db = db();
    let r = recipe(&mut db, "Soup");
    let rec = edit_recipe(
        &mut db,
        r,
        &RecipeEdit {
            photos: Some(vec![PHOTO.into()]),
            ..RecipeEdit::default()
        },
    )
    .unwrap();
    assert_eq!(rec.photos, [PHOTO]);
    let s = add_step(&mut db, r, "a").unwrap();
    let s = edit_step(
        &mut db,
        s.id,
        &StepEdit {
            photos: Some(vec!["fedcba9876543210.png".into(), PHOTO.into()]),
            ..StepEdit::default()
        },
    )
    .unwrap();
    assert_eq!(s.photos.len(), 2);
    let cleared = edit_recipe(
        &mut db,
        r,
        &RecipeEdit {
            photos: Some(vec![]),
            ..RecipeEdit::default()
        },
    )
    .unwrap();
    assert!(cleared.photos.is_empty());
}

#[test]
fn a_photo_name_this_app_did_not_make_is_refused() {
    let mut db = db();
    let r = recipe(&mut db, "Soup");
    for bad in ["../../etc/passwd", "photo.png", "/tmp/x.jpg"] {
        let e = edit_recipe(
            &mut db,
            r,
            &RecipeEdit {
                photos: Some(vec![bad.into()]),
                ..RecipeEdit::default()
            },
        );
        assert!(e.is_err(), "{bad}");
    }
    let s = add_step(&mut db, r, "a").unwrap();
    assert!(
        edit_step(
            &mut db,
            s.id,
            &StepEdit {
                photos: Some(vec!["nope.png".into()]),
                ..StepEdit::default()
            }
        )
        .is_err()
    );
}

fn scratch(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("recipes-photos-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn importing_copies_the_picture_once_by_content_and_refuses_what_is_not_a_picture() {
    let root = scratch("import");
    let dir = root.join("photos");
    let src = root.join("Dinner.JPEG");
    std::fs::write(&src, b"not really a jpeg but bytes").unwrap();
    let name = photos::import(&dir, &src).unwrap();
    assert!(name.ends_with(".jpg") && recipes_schema::photo::is_valid_name(&name));
    assert_eq!(
        std::fs::read(photos::path(&dir, &name).unwrap()).unwrap(),
        b"not really a jpeg but bytes"
    );
    // The same contents again, from another file: the same name, one copy.
    let again = root.join("copy.png.jpg");
    std::fs::copy(&src, &again).unwrap();
    assert_eq!(photos::import(&dir, &again).unwrap(), name);
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    let text = root.join("notes.txt");
    std::fs::write(&text, "hi").unwrap();
    assert!(
        photos::import(&dir, &text)
            .unwrap_err()
            .to_string()
            .contains("not a picture")
    );
    assert!(photos::import(&dir, &root.join("missing.png")).is_err());
    assert_eq!(photos::path(&dir, "../x.png"), None);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn clearing_unused_photos_keeps_what_a_recipe_or_a_step_lists_and_leaves_other_files_alone() {
    let root = scratch("gc");
    let dir = root.join("photos");
    let mut db = db();
    let r = recipe(&mut db, "Soup");
    let mk = |bytes: &[u8]| {
        let f = root.join(format!("{}.png", bytes.len()));
        std::fs::write(&f, bytes).unwrap();
        photos::import(&dir, &f).unwrap()
    };
    let (on_recipe, on_step, orphan) = (mk(b"a"), mk(b"bb"), mk(b"ccc"));
    edit_recipe(
        &mut db,
        r,
        &RecipeEdit {
            photos: Some(vec![on_recipe.clone()]),
            ..RecipeEdit::default()
        },
    )
    .unwrap();
    let s = add_step(&mut db, r, "x").unwrap();
    edit_step(
        &mut db,
        s.id,
        &StepEdit {
            photos: Some(vec![on_step.clone()]),
            ..StepEdit::default()
        },
    )
    .unwrap();
    std::fs::write(dir.join("readme.txt"), "mine").unwrap();
    assert_eq!(photos::gc(&db, &dir).unwrap(), 1);
    assert!(!dir.join(&orphan).exists());
    assert!(dir.join(&on_recipe).exists() && dir.join(&on_step).exists());
    assert!(
        dir.join("readme.txt").exists(),
        "files that are not ours stay"
    );
    // A missing folder is nothing to clear (a folder on disk, not a recipe folder).
    assert_eq!(photos::gc(&db, &root.join("none")).unwrap(), 0);
    let _ = std::fs::remove_dir_all(&root);
}

// ---- finding ---------------------------------------------------------------------------------

#[test]
fn a_recipe_is_found_by_its_title_ingredients_note_or_the_text_of_a_step() {
    let mut db = db();
    let soup = add_recipe(
        &mut db,
        &NewRecipe {
            notes: Some("grandma's favourite".into()),
            ..NewRecipe::new("Leek soup")
        },
    )
    .unwrap();
    let cake = add_recipe(&mut db, &NewRecipe::new("Cake")).unwrap();
    add_ingredient(&mut db, soup.id, "potatoes", Some(500.0), "g", false).unwrap();
    add_step(&mut db, soup.id, "Simmer for twenty minutes").unwrap();
    add_step(&mut db, cake.id, "Whisk the eggs").unwrap();
    let find = |q: &str| {
        let f = Filter {
            text: Some(q.into()),
            ..Filter::default()
        };
        recipes(&db, &f)
            .unwrap()
            .into_iter()
            .map(|r| r.title)
            .collect::<Vec<_>>()
    };
    assert_eq!(find("cake"), ["Cake"], "title");
    assert_eq!(find("potatoes"), ["Leek soup"], "ingredients");
    assert_eq!(find("grandma"), ["Leek soup"], "the personal note");
    assert_eq!(find("whisk"), ["Cake"], "a step");
    assert_eq!(find("simmer"), ["Leek soup"], "a step of another recipe");
    assert!(find("zzz").is_empty());
    assert_eq!(find("  ").len(), 2, "blank text is no search");
}

#[test]
fn searching_by_step_text_still_obeys_the_other_filters() {
    let mut db = db();
    let r1 = add_recipe(&mut db, &NewRecipe::new("one")).unwrap();
    let r2 = add_recipe(&mut db, &NewRecipe::new("two")).unwrap();
    add_step(&mut db, r1.id, "boil").unwrap();
    add_step(&mut db, r2.id, "boil").unwrap();
    set_favorite(&mut db, r2.id, true).unwrap();
    let f = Filter {
        text: Some("boil".into()),
        favorites: true,
        ..Filter::default()
    };
    assert_eq!(recipes(&db, &f).unwrap().len(), 1);
    assert_eq!(recipes(&db, &f).unwrap()[0].title, "two");
}

// ---- ingredients -----------------------------------------------------------------------------

#[test]
fn ingredients_keep_quantity_unit_and_whether_they_are_optional_and_stay_apart_from_steps() {
    let mut db = db();
    let soup = recipe(&mut db, "Soup");
    let leek = add_ingredient(&mut db, soup, " leeks ", Some(3.0), "", false).unwrap();
    let salt = add_ingredient(&mut db, soup, "salt", None, "", false).unwrap();
    let herb = add_ingredient(&mut db, soup, "parsley", Some(1.0), "bunch", true).unwrap();
    add_step(&mut db, soup, "Chop").unwrap();
    assert_eq!(leek.name, "leeks", "trimmed");
    assert_eq!((salt.quantity, herb.optional), (None, true));
    let all = ingredients(&db, soup).unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(
        steps(&db, soup).unwrap().len(),
        1,
        "ingredients are not steps"
    );

    let e = edit_ingredient(
        &mut db,
        salt.id,
        &IngredientEdit {
            quantity: Some(Some(1.0)),
            unit: Some("tsp".into()),
            optional: Some(true),
            ..IngredientEdit::default()
        },
    )
    .unwrap();
    assert_eq!(
        (e.quantity, e.unit.as_str(), e.optional),
        (Some(1.0), "tsp", true)
    );
    let e = edit_ingredient(
        &mut db,
        salt.id,
        &IngredientEdit {
            quantity: Some(None),
            ..IngredientEdit::default()
        },
    )
    .unwrap();
    assert_eq!(e.quantity, None, "cleared back to to-taste");

    assert!(
        add_ingredient(&mut db, soup, "  ", None, "", false).is_err(),
        "needs a name"
    );
    assert!(add_ingredient(&mut db, soup, "x", Some(-1.0), "", false).is_err());
    assert!(add_ingredient(&mut db, soup, "x", Some(f64::NAN), "", false).is_err());
    let fridge = add_stock(&mut db, "milk", None, "", None).unwrap().id;
    assert!(
        add_ingredient(&mut db, fridge, "x", None, "", false).is_err(),
        "only recipes have them"
    );
    assert!(delete_ingredient(&mut db, soup).is_err());
    delete_ingredient(&mut db, leek.id).unwrap();
    assert_eq!(ingredients(&db, soup).unwrap().len(), 2);
}

#[test]
fn ingredients_move_among_ingredients_even_with_steps_between() {
    let mut db = db();
    let soup = recipe(&mut db, "Soup");
    let a = add_ingredient(&mut db, soup, "a", None, "", false).unwrap();
    add_step(&mut db, soup, "step").unwrap();
    add_ingredient(&mut db, soup, "b", None, "", false).unwrap();
    let c = add_ingredient(&mut db, soup, "c", None, "", false).unwrap();
    let names = |db: &Db| {
        ingredients(db, soup)
            .unwrap()
            .into_iter()
            .map(|i| i.name)
            .collect::<Vec<_>>()
            .join("")
    };
    move_ingredient(&mut db, c.id, -1).unwrap();
    assert_eq!(names(&db), "acb");
    move_ingredient(&mut db, a.id, -1).unwrap();
    assert_eq!(names(&db), "acb", "at the top nothing changes");
    move_ingredient(&mut db, a.id, 1).unwrap();
    assert_eq!(names(&db), "cab");
    assert_eq!(steps(&db, soup).unwrap().len(), 1);
}

#[test]
fn searching_finds_a_recipe_by_what_it_needs() {
    let mut db = db();
    let soup = recipe(&mut db, "Soup");
    add_ingredient(&mut db, soup, "leeks", Some(3.0), "", false).unwrap();
    let f = Filter {
        text: Some("leek".into()),
        ..Filter::default()
    };
    assert_eq!(recipes(&db, &f).unwrap().len(), 1);
}

// ---- the fridge and what is missing ----------------------------------------------------------

#[test]
fn the_fridge_adds_up_what_is_put_in_twice_and_can_be_corrected() {
    let mut db = db();
    add_stock(&mut db, "Eggs", Some(6.0), "", None).unwrap();
    let e = add_stock(&mut db, "eggs", Some(4.0), "", None).unwrap();
    assert_eq!(e.quantity, Some(10.0));
    add_stock(&mut db, "flour", Some(1.0), "kg", None).unwrap();
    add_stock(&mut db, "flour", Some(500.0), "g", None).unwrap();
    assert_eq!(stock(&db).unwrap().len(), 3, "another unit is another line");
    let e = edit_stock(&mut db, e.id, Some(Some(2.0)), None, None).unwrap();
    assert_eq!(e.quantity, Some(2.0));
    assert!(add_stock(&mut db, "x", Some(-3.0), "", None).is_err());
    assert!(add_stock(&mut db, " ", None, "", None).is_err());
    let r = rename_stock(&mut db, e.id, " Free-range eggs ").unwrap();
    assert_eq!(
        (r.name.as_str(), r.quantity),
        ("Free-range eggs", Some(2.0))
    );
    assert!(
        rename_stock(&mut db, e.id, "  ").is_err(),
        "a name is needed"
    );
}

// ---- the remembered shopping list -------------------------------------------------------------

#[test]
fn the_shopping_list_is_remembered_with_when_it_last_changed() {
    let mut db = db();
    assert_eq!(
        shopping_picks(&db).unwrap(),
        Default::default(),
        "nothing yet"
    );
    let list = save_shopping(&mut db, &[(7, 4.0), (9, 2.5)]).unwrap();
    assert_eq!(list.picks, [(7, 4.0), (9, 2.5)]);
    let first = shopping_picks(&db).unwrap();
    assert_eq!(first.picks, list.picks);
    assert!(first.updated.is_some());
    // The same picks again change nothing, not even the date; other picks replace them.
    assert_eq!(
        save_shopping(&mut db, &[(7, 4.0), (9, 2.5)])
            .unwrap()
            .updated,
        first.updated
    );
    save_shopping(&mut db, &[(7, 8.0)]).unwrap();
    assert_eq!(shopping_picks(&db).unwrap().picks, [(7, 8.0)]);
    save_shopping(&mut db, &[]).unwrap();
    let cleared = shopping_picks(&db).unwrap();
    assert!(
        cleared.picks.is_empty() && cleared.updated.is_some(),
        "cleared, but remembered"
    );
    assert_eq!(
        db.list(&organiser_engine::Query {
            kind: Some("shoplist".into()),
            ..Default::default()
        })
        .unwrap()
        .len(),
        1,
        "always one item"
    );
}

// ---- the shopping list ------------------------------------------------------------------------

#[test]
fn a_shopping_list_adds_up_the_picked_recipes_and_takes_off_the_fridge() {
    let mut db = db();
    let pasta = recipe(&mut db, "Pasta");
    let tart = add_recipe(&mut db, &NewRecipe::new("Onion tart"))
        .unwrap()
        .id;
    add_ingredient(&mut db, pasta, "pasta", Some(100.0), "g", false).unwrap();
    add_ingredient(&mut db, pasta, "onion", Some(1.0), "", false).unwrap();
    add_ingredient(&mut db, pasta, "basil", Some(1.0), "bunch", true).unwrap();
    add_ingredient(&mut db, tart, "onion", Some(2.0), "", false).unwrap();
    add_stock(&mut db, "Pasta", Some(0.25), "kg", None).unwrap();
    add_stock(&mut db, "onions", Some(4.0), "", None).unwrap();
    // Pasta for five portions (x5) and one tart: 500 g pasta (250 on hand), 5 + 2 onions (4 on hand).
    let list = shopping_list(&db, &[(pasta, 5.0), (tart, 1.0)], day("2026-11-11"), false).unwrap();
    let to_get: Vec<_> = list.iter().filter(|i| i.status != Status::Enough).collect();
    assert_eq!(to_get.len(), 2, "basil is optional: {list:?}");
    assert_eq!(to_get[0].get, Some((250.0, "g".to_string())));
    assert_eq!(to_get[1].get, Some((3.0, String::new())));
    let md = shopping_markdown(&list);
    assert_eq!(md, "- [ ] 250 g pasta\n- [ ] 3 onion");
    // A recipe that is gone counts for nothing.
    assert!(
        shopping_list(&db, &[(9999, 2.0)], day("2026-11-11"), false)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn the_fridge_keeps_its_saved_order_and_expired_food_does_not_count() {
    let mut db = db();
    add_stock(&mut db, "rice", Some(1.0), "kg", None).unwrap();
    let milk = add_stock(&mut db, "milk", Some(1.0), "l", Some(day("2026-11-10"))).unwrap();
    add_stock(&mut db, "eggs", Some(6.0), "", Some(day("2026-11-20"))).unwrap();
    let names: Vec<_> = stock(&db).unwrap().into_iter().map(|s| s.name).collect();
    assert_eq!(
        names,
        ["rice", "milk", "eggs"],
        "the order they were saved in"
    );
    assert!(milk.is_expired(day("2026-11-11")) && !milk.is_expired(day("2026-11-10")));
    // Another batch with another date is its own line; the same date adds up.
    add_stock(&mut db, "milk", Some(1.0), "l", Some(day("2026-11-25"))).unwrap();
    add_stock(&mut db, "milk", Some(1.0), "l", Some(day("2026-11-25"))).unwrap();
    assert_eq!(stock(&db).unwrap().len(), 4);
    // Expired milk is not something you have.
    let pancakes = recipe(&mut db, "Pancakes");
    add_ingredient(&mut db, pancakes, "milk", Some(3.0), "l", false).unwrap();
    let list = shopping_list(&db, &[(pancakes, 1.0)], day("2026-11-11"), false).unwrap();
    assert_eq!(
        list[0].get,
        Some((1.0, "l".to_string())),
        "3 wanted, 2 fresh"
    );
    // The date can be changed or cleared.
    let m = edit_stock(&mut db, milk.id, None, None, Some(None)).unwrap();
    assert_eq!(m.expires, None);
}

#[test]
fn servings_are_a_positive_number_and_can_be_cleared() {
    let mut db = db();
    let r = add_recipe(
        &mut db,
        &NewRecipe {
            servings: Some(4.0),
            ..NewRecipe::new("Stew")
        },
    )
    .unwrap();
    assert_eq!(r.servings, Some(4.0));
    assert!(
        add_recipe(
            &mut db,
            &NewRecipe {
                servings: Some(0.0),
                ..NewRecipe::new("x")
            }
        )
        .is_err()
    );
    let r = edit_recipe(
        &mut db,
        r.id,
        &RecipeEdit {
            servings: Some(None),
            ..RecipeEdit::default()
        },
    )
    .unwrap();
    assert_eq!(r.servings, None);
}

#[test]
fn fridge_items_can_be_moved_and_the_order_is_kept() {
    let mut db = db();
    let ids: Vec<i64> = ["a", "b", "c", "d"]
        .iter()
        .map(|n| add_stock(&mut db, n, None, "", None).unwrap().id)
        .collect();
    // Other things at the top level (a meal, the shopping list) must not get in the way.
    let soup = recipe(&mut db, "Soup");
    add_meal(&mut db, soup, d("2026-11-03"), None, 1.0, "").unwrap();
    save_shopping(&mut db, &[(soup, 1.0)]).unwrap();
    let order = |db: &Db| {
        stock(db)
            .unwrap()
            .into_iter()
            .map(|s| s.name)
            .collect::<Vec<_>>()
            .join("")
    };
    move_stock(&mut db, ids[3], -3).unwrap();
    assert_eq!(order(&db), "dabc");
    move_stock(&mut db, ids[3], 1).unwrap();
    assert_eq!(order(&db), "adbc");
    move_stock(&mut db, ids[3], 1).unwrap();
    assert_eq!(order(&db), "abdc");
    move_stock(&mut db, ids[3], 5).unwrap();
    assert_eq!(order(&db), "abdc", "past the end changes nothing");
    assert!(move_stock(&mut db, soup, 1).is_err(), "only fridge items");
}

// ---- the calendar ----------------------------------------------------------------------------

#[test]
fn meals_have_a_date_multiplier_and_note_and_are_read_by_range_or_recipe() {
    let mut db = db();
    let soup = recipe(&mut db, "Soup");
    let m = add_meal(
        &mut db,
        soup,
        d("2026-11-03"),
        Some(MealTime::Lunch),
        1.0,
        "too salty",
    )
    .unwrap();
    assert_eq!(m.time, Some(MealTime::Lunch));
    add_meal(&mut db, soup, d("2026-11-20"), None, 5.0, "").unwrap();
    add_meal(&mut db, soup, d("2026-12-01"), None, 1.0, "xmas").unwrap();
    let nov = meals_between(&db, d("2026-11-01"), d("2026-11-30")).unwrap();
    assert_eq!(
        nov.iter().map(|c| c.date.to_string()).collect::<Vec<_>>(),
        ["2026-11-03", "2026-11-20"]
    );
    assert_eq!(
        (nov[0].note.as_str(), nov[0].title.as_str()),
        ("too salty", "Soup")
    );
    assert_eq!(nov[1].servings, 5.0);
    assert_eq!(
        meals_between(&db, d("2026-11-20"), d("2026-11-20"))
            .unwrap()
            .len(),
        1,
        "inclusive"
    );
    let all = meals_for(&db, soup).unwrap();
    assert_eq!(
        all.iter().map(|c| c.date.to_string()).collect::<Vec<_>>(),
        ["2026-12-01", "2026-11-20", "2026-11-03"],
        "newest first"
    );
    assert!(nov[0].is_planned(d("2026-11-02")) && !nov[0].is_planned(d("2026-11-03")));
}

#[test]
fn a_meal_can_be_moved_rescaled_or_removed_and_servings_stay_at_least_one() {
    let mut db = db();
    let a = recipe(&mut db, "Soup");
    let b = add_recipe(&mut db, &NewRecipe::new("Bread")).unwrap().id;
    let c1 = add_meal(&mut db, a, d("2026-11-03"), None, 1.0, "").unwrap();
    add_meal(&mut db, b, d("2026-11-03"), None, 1.0, "").unwrap();
    assert_eq!(
        meals_between(&db, d("2026-11-03"), d("2026-11-03"))
            .unwrap()
            .len(),
        2
    );
    let moved = edit_meal(
        &mut db,
        c1.id,
        Some(d("2026-11-04")),
        Some(Some(MealTime::Dinner)),
        Some(3.0),
        Some("better"),
    )
    .unwrap();
    assert_eq!(moved.time, Some(MealTime::Dinner));
    let cleared = edit_meal(&mut db, c1.id, None, Some(None), None, None).unwrap();
    assert_eq!(cleared.time, None, "the time of day can be cleared");
    assert_eq!(
        (moved.date, moved.servings, moved.note.as_str()),
        (d("2026-11-04"), 3.0, "better")
    );
    assert!(edit_meal(&mut db, c1.id, None, None, Some(0.0), None).is_err());
    assert!(add_meal(&mut db, a, d("2026-11-05"), None, 0.5, "").is_err());
    assert!(add_meal(&mut db, a, d("2026-11-05"), None, f64::NAN, "").is_err());
    delete_meal(&mut db, c1.id).unwrap();
    assert_eq!(
        meals_between(&db, d("2026-11-01"), d("2026-11-30"))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn the_calendar_outlives_the_recipe_and_keeps_its_title() {
    let mut db = db();
    let soup = recipe(&mut db, "Soup");
    add_meal(&mut db, soup, d("2026-11-03"), None, 1.0, "good").unwrap();
    db.delete(soup).unwrap();
    let left = meals_between(&db, d("2026-11-01"), d("2026-11-30")).unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!((left[0].title.as_str(), left[0].recipe_id), ("Soup", None));
}

#[test]
fn only_a_recipe_can_be_put_on_the_calendar() {
    let mut db = db();
    let soup = recipe(&mut db, "Soup");
    let fridge = add_stock(&mut db, "milk", None, "", None).unwrap().id;
    assert!(add_meal(&mut db, fridge, d("2026-11-03"), None, 1.0, "").is_err());
    assert!(
        delete_meal(&mut db, soup).is_err(),
        "a recipe is not a meal"
    );
}
