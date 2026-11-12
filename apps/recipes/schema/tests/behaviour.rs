//! Recipes as entries: favourites, ratings, search, and tag filters.

use organiser_engine::{Db, TagFilter};
use recipes_schema::SCHEMA;
use recipes_schema::lifecycle::{NewRecipe, add_recipe, edit_recipe, set_favorite, set_tags};
use recipes_schema::model::{Recipe, RecipeEdit};
use recipes_schema::views::{Filter, Sort, recipes};

fn db() -> Db {
    Db::open_in_memory(&SCHEMA).unwrap()
}

fn titles(v: &[Recipe]) -> Vec<&str> {
    v.iter().map(|e| e.title.as_str()).collect()
}

#[test]
fn a_rating_is_one_to_five_and_can_be_cleared() {
    let mut db = db();
    let with = |rating| NewRecipe {
        rating: Some(rating),
        ..NewRecipe::new("x")
    };
    assert_eq!(add_recipe(&mut db, &with(3)).unwrap().rating, Some(3));
    assert!(add_recipe(&mut db, &with(9)).is_err(), "1 to 5 only");
    let r = add_recipe(&mut db, &with(5)).unwrap();
    let r = edit_recipe(
        &mut db,
        r.id,
        &RecipeEdit {
            rating: Some(None),
            ..RecipeEdit::default()
        },
    )
    .unwrap();
    assert_eq!(r.rating, None);
}

#[test]
fn favorites_sort_by_rating_with_unrated_last() {
    let mut db = db();
    let a = add_recipe(
        &mut db,
        &NewRecipe {
            rating: Some(3),
            ..NewRecipe::new("Okay")
        },
    )
    .unwrap();
    let b = add_recipe(
        &mut db,
        &NewRecipe {
            rating: Some(5),
            ..NewRecipe::new("Great")
        },
    )
    .unwrap();
    let c = add_recipe(&mut db, &NewRecipe::new("Plain")).unwrap();
    add_recipe(&mut db, &NewRecipe::new("Skipped")).unwrap();
    for r in [&a, &b, &c] {
        set_favorite(&mut db, r.id, true).unwrap();
    }
    let rated = recipes(
        &db,
        &Filter {
            favorites: true,
            sort: Sort::Rating,
            ..Filter::default()
        },
    )
    .unwrap();
    assert_eq!(titles(&rated), ["Great", "Okay", "Plain"]);
}

#[test]
fn search_finds_by_title_or_note_and_tags_narrow() {
    let mut db = db();
    let a = add_recipe(
        &mut db,
        &NewRecipe {
            notes: Some("Spooky space horror".into()),
            ..NewRecipe::new("Alien stew")
        },
    )
    .unwrap();
    add_recipe(&mut db, &NewRecipe::new("Amelie soup")).unwrap();
    set_tags(&mut db, a.id, &["dinner".into(), "spicy".into()]).unwrap();
    let find = |f: Filter| titles(&recipes(&db, &f).unwrap()).join(",");
    assert_eq!(
        find(Filter {
            text: Some("spooky".into()),
            ..Filter::default()
        }),
        "Alien stew",
        "notes are searched"
    );
    assert_eq!(
        find(Filter {
            tags: TagFilter {
                all: vec!["dinner".into()],
                ..Default::default()
            },
            ..Filter::default()
        }),
        "Alien stew"
    );
}

#[test]
fn tag_groups_are_alternatives_and_a_group_needs_all_its_tags() {
    let mut db = db();
    let add = |db: &mut Db, title: &str, tags: &[&str]| {
        let r = add_recipe(db, &NewRecipe::new(title)).unwrap();
        set_tags(
            db,
            r.id,
            &tags.iter().map(|t| t.to_string()).collect::<Vec<_>>(),
        )
        .unwrap();
    };
    add(&mut db, "Curry", &["dinner", "spicy"]);
    add(&mut db, "Toast", &["breakfast"]);
    add(&mut db, "Salad", &["dinner"]);
    let group = |tags: &[&str]| TagFilter {
        all: tags.iter().map(|t| t.to_string()).collect(),
        ..TagFilter::default()
    };
    let either = TagFilter {
        blocks: vec![vec![group(&["dinner", "spicy"]), group(&["breakfast"])]],
        ..TagFilter::default()
    };
    let f = Filter {
        tags: either,
        sort: Sort::Title,
        ..Filter::default()
    };
    assert_eq!(titles(&recipes(&db, &f).unwrap()), ["Curry", "Toast"]);
}
