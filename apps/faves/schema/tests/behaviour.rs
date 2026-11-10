use faves_schema::SCHEMA;
use faves_schema::lifecycle::{
    NewEntry, add_entry, create_list, delete_list, edit_entry, edit_list, move_entry, set_favorite,
    set_tags,
};
use faves_schema::model::{Entry, EntryEdit, ListEdit};
use faves_schema::status::Status;
use faves_schema::views::{Filter, Sort, entries, lists};
use organiser_engine::Db;

fn db() -> Db {
    Db::open_in_memory(&SCHEMA).unwrap()
}

fn titles(v: &[Entry]) -> Vec<&str> {
    v.iter().map(|e| e.title.as_str()).collect()
}

#[test]
fn rating_is_an_option_each_list_chooses() {
    let mut db = db();
    let movies = create_list(&mut db, "Movies", true, false).unwrap();
    let animals = create_list(&mut db, "Animals", false, false).unwrap();

    let alien = add_entry(
        &mut db,
        movies.id,
        &NewEntry {
            rating: Some(5),
            ..NewEntry::new("Alien")
        },
    )
    .unwrap();
    assert_eq!(alien.rating, Some(5));
    assert_eq!(alien.list_id, movies.id);

    // Animals cannot be rated; the engine refuses, whatever frontend asks.
    assert!(
        add_entry(
            &mut db,
            animals.id,
            &NewEntry {
                rating: Some(4),
                ..NewEntry::new("Otter")
            }
        )
        .is_err()
    );
    let otter = add_entry(&mut db, animals.id, &NewEntry::new("Otter")).unwrap();
    assert_eq!(otter.rating, None);
    assert!(
        edit_entry(
            &mut db,
            otter.id,
            &EntryEdit {
                rating: Some(Some(3)),
                ..EntryEdit::default()
            }
        )
        .is_err()
    );
}

#[test]
fn switching_rating_off_removes_the_ratings_and_back_on_allows_new_ones() {
    let mut db = db();
    let movies = create_list(&mut db, "Movies", true, false).unwrap();
    let a = add_entry(
        &mut db,
        movies.id,
        &NewEntry {
            rating: Some(5),
            notes: Some("Great.".into()),
            ..NewEntry::new("A")
        },
    )
    .unwrap();
    edit_list(
        &mut db,
        movies.id,
        &ListEdit {
            rated: Some(false),
            ..ListEdit::default()
        },
    )
    .unwrap();
    let a = db.get(a.id).map(|i| Entry::from(&i)).unwrap();
    assert_eq!(
        (a.rating, a.notes.as_str()),
        (None, "Great."),
        "only the rating went"
    );
    edit_list(
        &mut db,
        movies.id,
        &ListEdit {
            rated: Some(true),
            ..ListEdit::default()
        },
    )
    .unwrap();
    let a = edit_entry(
        &mut db,
        a.id,
        &EntryEdit {
            rating: Some(Some(4)),
            ..EntryEdit::default()
        },
    )
    .unwrap();
    assert_eq!(a.rating, Some(4));
}

#[test]
fn status_is_a_separate_option_and_ratings_stay_in_range() {
    let mut db = db();
    let shows = create_list(&mut db, "Shows", false, true).unwrap();
    let cars = create_list(&mut db, "Cars", false, false).unwrap();
    let e = add_entry(
        &mut db,
        shows.id,
        &NewEntry {
            status: Some(Status::Active),
            ..NewEntry::new("Severance")
        },
    )
    .unwrap();
    assert_eq!(e.status, Some(Status::Active));
    assert!(
        add_entry(
            &mut db,
            cars.id,
            &NewEntry {
                status: Some(Status::Done),
                ..NewEntry::new("Mini")
            }
        )
        .is_err()
    );
    assert!(
        add_entry(
            &mut db,
            shows.id,
            &NewEntry {
                rating: Some(3),
                ..NewEntry::new("x")
            }
        )
        .is_err(),
        "shows are not rated"
    );
    let rated = create_list(&mut db, "Rated", true, false).unwrap();
    assert!(
        add_entry(
            &mut db,
            rated.id,
            &NewEntry {
                rating: Some(9),
                ..NewEntry::new("x")
            }
        )
        .is_err(),
        "1 to 5 only"
    );
}

#[test]
fn deleting_a_list_deletes_its_entries() {
    let mut db = db();
    let l = create_list(&mut db, "Cars", false, false).unwrap();
    let other = create_list(&mut db, "Animals", false, false).unwrap();
    add_entry(&mut db, l.id, &NewEntry::new("Mini")).unwrap();
    add_entry(&mut db, l.id, &NewEntry::new("Beetle")).unwrap();
    add_entry(&mut db, other.id, &NewEntry::new("Otter")).unwrap();
    assert_eq!(delete_list(&mut db, l.id).unwrap(), 3);
    let left = entries(&db, &Filter::default()).unwrap();
    assert_eq!(titles(&left), ["Otter"]);
    assert!(
        delete_list(&mut db, left[0].id).is_err(),
        "an entry is not a list"
    );
}

#[test]
fn moving_an_entry_drops_what_the_new_list_does_not_offer() {
    let mut db = db();
    let movies = create_list(&mut db, "Movies", true, true).unwrap();
    let docs = create_list(&mut db, "Documentaries", false, false).unwrap();
    let e = add_entry(
        &mut db,
        movies.id,
        &NewEntry {
            rating: Some(5),
            status: Some(Status::Done),
            ..NewEntry::new("Free Solo")
        },
    )
    .unwrap();
    let moved = move_entry(&mut db, e.id, docs.id).unwrap();
    assert_eq!(
        (moved.list_id, moved.rating, moved.status),
        (docs.id, None, None)
    );
    // The engine itself also refuses to move a rated entry somewhere ratings are off.
    let again = add_entry(
        &mut db,
        movies.id,
        &NewEntry {
            rating: Some(4),
            ..NewEntry::new("Alien")
        },
    )
    .unwrap();
    assert!(db.move_item(again.id, Some(docs.id), None).is_err());
}

#[test]
fn favorites_span_lists_and_sort_by_rating() {
    let mut db = db();
    let movies = create_list(&mut db, "Movies", true, false).unwrap();
    let animals = create_list(&mut db, "Animals", false, false).unwrap();
    let a = add_entry(
        &mut db,
        movies.id,
        &NewEntry {
            rating: Some(3),
            ..NewEntry::new("Okay film")
        },
    )
    .unwrap();
    let b = add_entry(
        &mut db,
        movies.id,
        &NewEntry {
            rating: Some(5),
            ..NewEntry::new("Great film")
        },
    )
    .unwrap();
    let c = add_entry(&mut db, animals.id, &NewEntry::new("Otter")).unwrap();
    add_entry(&mut db, animals.id, &NewEntry::new("Skipped")).unwrap();
    for e in [&a, &b, &c] {
        set_favorite(&mut db, e.id, true).unwrap();
    }
    let faves = entries(
        &db,
        &Filter {
            favorites: true,
            sort: Sort::Rating,
            ..Filter::default()
        },
    )
    .unwrap();
    assert_eq!(
        titles(&faves),
        ["Great film", "Okay film", "Otter"],
        "unrated sorts last"
    );
    let one = entries(
        &db,
        &Filter {
            list: Some(animals.id),
            favorites: true,
            ..Filter::default()
        },
    )
    .unwrap();
    assert_eq!(titles(&one), ["Otter"]);
}

#[test]
fn search_tags_and_summaries() {
    let mut db = db();
    let movies = create_list(&mut db, "Movies", true, true).unwrap();
    let a = add_entry(
        &mut db,
        movies.id,
        &NewEntry {
            notes: Some("Spooky space horror".into()),
            status: Some(Status::Done),
            ..NewEntry::new("Alien")
        },
    )
    .unwrap();
    add_entry(&mut db, movies.id, &NewEntry::new("Amelie")).unwrap();
    set_tags(&mut db, a.id, &["scifi".into(), "horror".into()]).unwrap();
    set_favorite(&mut db, a.id, true).unwrap();

    let found = entries(
        &db,
        &Filter {
            text: Some("spooky".into()),
            ..Filter::default()
        },
    )
    .unwrap();
    assert_eq!(titles(&found), ["Alien"], "notes are searched");
    let tagged = entries(
        &db,
        &Filter {
            tags: organiser_engine::TagFilter {
                all: vec!["scifi".into()],
                ..Default::default()
            },
            ..Filter::default()
        },
    )
    .unwrap();
    assert_eq!(titles(&tagged), ["Alien"]);
    let summary = &lists(&db).unwrap()[0];
    assert_eq!(
        (summary.entries, summary.favorites, summary.done),
        (2, 1, 1)
    );
}

#[test]
fn filters_combine_rating_favourite_and_tag_groups() {
    use organiser_engine::TagFilter;
    let mut db = db();
    let movies = create_list(&mut db, "Movies", true, false).unwrap();
    let add = |db: &mut Db, title: &str, rating: Option<u8>, fave: bool, tags: &[&str]| {
        let e = add_entry(
            db,
            movies.id,
            &NewEntry {
                rating,
                ..NewEntry::new(title)
            },
        )
        .unwrap();
        set_tags(
            db,
            e.id,
            &tags.iter().map(|t| t.to_string()).collect::<Vec<_>>(),
        )
        .unwrap();
        if fave {
            set_favorite(db, e.id, true).unwrap();
        }
    };
    add(&mut db, "Alien", Some(5), true, &["scifi", "horror"]);
    add(&mut db, "Dune", Some(3), true, &["scifi"]);
    add(&mut db, "Amelie", Some(4), false, &["french"]);
    add(&mut db, "Unrated", None, true, &["scifi"]);

    let found = |f: Filter| titles(&entries(&db, &f).unwrap()).join(",");
    let all = || Filter {
        sort: Sort::Title,
        ..Filter::default()
    };
    assert_eq!(
        found(Filter {
            min_rating: Some(4),
            ..all()
        }),
        "Alien,Amelie",
        "unrated never matches"
    );
    assert_eq!(
        found(Filter {
            favorites: true,
            ..all()
        }),
        "Alien,Dune,Unrated"
    );
    assert_eq!(
        found(Filter {
            favorites: true,
            min_rating: Some(4),
            ..all()
        }),
        "Alien"
    );

    // Groups are alternatives and a group needs all its tags: (scifi AND horror) OR french.
    let group = |tags: &[&str]| TagFilter {
        all: tags.iter().map(|t| t.to_string()).collect(),
        ..TagFilter::default()
    };
    let either = TagFilter {
        blocks: vec![vec![group(&["scifi", "horror"]), group(&["french"])]],
        ..TagFilter::default()
    };
    assert_eq!(
        found(Filter {
            tags: either,
            ..all()
        }),
        "Alien,Amelie"
    );
    let one = TagFilter {
        blocks: vec![vec![group(&["scifi"])]],
        ..TagFilter::default()
    };
    assert_eq!(
        found(Filter {
            tags: one.clone(),
            min_rating: Some(4),
            ..all()
        }),
        "Alien"
    );
    assert_eq!(
        found(Filter {
            tags: one,
            favorites: true,
            ..all()
        }),
        "Alien,Dune,Unrated"
    );
}
