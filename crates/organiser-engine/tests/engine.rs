use organiser_engine::*;
use serde_json::json;

static SCHEMA: Schema = Schema {
    kinds: &[
        Kind {
            name: "task",
            fields: &[
                Field::new("notes", FieldType::Markdown).searchable(),
                Field::new("status", FieldType::Enum(&["open", "done", "dismissed"]))
                    .indexed()
                    .default_to("open"),
                Field::new("due", FieldType::Date).indexed(),
                Field::new("steps", FieldType::Checklist).searchable(),
                Field::new("link", FieldType::Reference("link")).indexed(),
            ],
        },
        Kind {
            name: "link",
            fields: &[
                Field::new("url", FieldType::Url).required(),
                Field::new("stars", FieldType::Rating).indexed(),
            ],
        },
    ],
};

fn db() -> Db {
    Db::open_in_memory(&SCHEMA).unwrap()
}

fn titles(items: &[Item]) -> Vec<&str> {
    items.iter().map(|i| i.title.as_str()).collect()
}

#[test]
fn create_get_roundtrip() {
    let mut db = db();
    let it = db
        .create(
            NewItem::new("task", "  Buy milk ")
                .field("status", "open")
                .field("due", "2026-11-12")
                .tag("home"),
        )
        .unwrap();
    assert_eq!(it.title, "Buy milk");
    assert_eq!(it.tags, vec!["home"]);
    assert_eq!(db.get(it.id).unwrap(), it);
    assert!(matches!(db.get(999), Err(Error::NotFound(999))));
}

#[test]
fn defaults_and_parse_input() {
    let mut db = db();
    let it = db.create(NewItem::new("task", "t")).unwrap();
    assert_eq!(it.fields["status"], "open");
    let k = SCHEMA.kind("link").unwrap();
    assert_eq!(k.field("stars").unwrap().parse_input(" 4 ").unwrap(), 4);
    assert!(k.field("stars").unwrap().parse_input("9").is_err());
    assert!(k.field("stars").unwrap().parse_input("x").is_err());
    let t = SCHEMA.kind("task").unwrap();
    assert!(t.field("due").unwrap().parse_input("tomorrow").is_err());
}

#[test]
fn validation_rejects_bad_input() {
    let mut db = db();
    let bad = [
        NewItem::new("nope", "x"),
        NewItem::new("task", "  "),
        NewItem::new("task", "x").field("status", "weird"),
        NewItem::new("task", "x").field("due", "2026-13-01"),
        NewItem::new("task", "x").field("ghost", 1),
        NewItem::new("link", "x"),
        NewItem::new("link", "x").field("url", "not a url"),
        NewItem::new("link", "x")
            .field("url", "https://a.b")
            .field("stars", 9),
        NewItem::new("task", "x").field("steps", json!([{"text": "a"}])),
        NewItem::new("task", "x").tag("=bad"),
        NewItem::new("task", "x").parent(42),
    ];
    for b in bad {
        assert!(db.create(b.clone()).is_err(), "{b:?}");
    }
    assert!(
        db.list(&Query::default()).unwrap().is_empty(),
        "failed creates leave nothing behind"
    );
}

#[test]
fn update_merges_and_null_removes() {
    let mut db = db();
    let it = db
        .create(
            NewItem::new("task", "a")
                .field("status", "open")
                .field("due", "2026-01-01"),
        )
        .unwrap();
    let u = db
        .update(
            it.id,
            ItemPatch::default()
                .title("b")
                .field("status", "done")
                .field("due", json!(null)),
        )
        .unwrap();
    assert_eq!(u.title, "b");
    assert_eq!(u.fields["status"], "done");
    assert!(!u.fields.contains_key("due"));
    assert!(
        db.update(it.id, ItemPatch::default().field("status", "x"))
            .is_err()
    );
    assert_eq!(db.get(it.id).unwrap().fields["status"], "done");
}

#[test]
fn hierarchy_move_cycle_and_cascade() {
    let mut db = db();
    let a = db.create(NewItem::new("task", "a")).unwrap();
    let b = db
        .create(NewItem::new("task", "b").parent(a.id).tag("x"))
        .unwrap();
    let c = db
        .create(
            NewItem::new("task", "c")
                .parent(b.id)
                .field("notes", "zebra"),
        )
        .unwrap();
    let d = db.create(NewItem::new("task", "d").parent(a.id)).unwrap();
    assert_eq!(titles(&db.children(Some(a.id)).unwrap()), ["b", "d"]);
    assert_eq!(titles(&db.children(None).unwrap()), ["a"]);

    assert!(db.move_item(a.id, Some(c.id), None).is_err());
    assert!(db.move_item(a.id, Some(a.id), None).is_err());
    db.move_item(d.id, Some(a.id), Some(0)).unwrap();
    assert_eq!(titles(&db.children(Some(a.id)).unwrap()), ["d", "b"]);
    db.move_item(c.id, None, None).unwrap();
    assert_eq!(db.get(c.id).unwrap().parent_id, None);
    db.move_item(c.id, Some(b.id), None).unwrap();

    assert_eq!(db.delete(a.id).unwrap(), 4);
    assert!(db.list(&Query::default()).unwrap().is_empty());
    let q = Query {
        text: Some("zebra".into()),
        ..Query::default()
    };
    assert!(db.list(&q).unwrap().is_empty(), "search index cleaned up");
    assert!(matches!(db.delete(a.id), Err(Error::NotFound(_))));
}

#[test]
fn search_covers_title_and_searchable_fields() {
    let mut db = db();
    db.create(NewItem::new("task", "Plan trip").field("notes", "book flights to Lisbon"))
        .unwrap();
    db.create(
        NewItem::new("task", "Taxes")
            .field("steps", json!([{"text": "gather receipts", "done": false}])),
    )
    .unwrap();
    fn find(db: &Db, t: &str) -> String {
        let q = Query {
            text: Some(t.into()),
            ..Query::default()
        };
        titles(&db.list(&q).unwrap()).join(",")
    }
    assert_eq!(find(&db, "lisb"), "Plan trip");
    assert_eq!(find(&db, "receipts"), "Taxes");
    assert_eq!(find(&db, "plan flights"), "Plan trip");
    assert_eq!(find(&db, "\"; DROP"), "");
    let id = db.list(&Query::default()).unwrap()[0].id;
    db.update(id, ItemPatch::default().title("Holiday"))
        .unwrap();
    assert_eq!(find(&db, "holiday"), "Holiday");
    assert_eq!(find(&db, "plan"), "");
}

#[test]
fn filters_sort_and_paging() {
    let mut db = db();
    for (t, s, due) in [
        ("a", "open", "2026-03-01"),
        ("b", "done", "2026-01-01"),
        ("c", "open", "2026-02-01"),
    ] {
        db.create(NewItem::new("task", t).field("status", s).field("due", due))
            .unwrap();
    }
    db.create(NewItem::new("task", "d")).unwrap();
    let q = |filters: Vec<FieldFilter>, sort: Vec<Sort>| {
        titles(
            &db.list(&Query {
                kind: Some("task".into()),
                filters,
                sort,
                ..Query::default()
            })
            .unwrap(),
        )
        .join("")
    };
    assert_eq!(
        q(
            vec![FieldFilter::new("status", Op::Eq, "open")],
            vec![Sort::asc("due")]
        ),
        "dca"
    ); // d defaults to open; NULL sorts first
    assert_eq!(
        q(
            vec![FieldFilter::new("due", Op::Lt, "2026-02-15")],
            vec![Sort::desc("due")]
        ),
        "cb"
    );
    assert_eq!(
        q(
            vec![FieldFilter::new("due", Op::IsNull, json!(null))],
            vec![]
        ),
        "d"
    );
    assert_eq!(
        q(vec![FieldFilter::new("title", Op::Contains, "%")], vec![]),
        ""
    );
    assert!(
        db.list(&Query {
            filters: vec![FieldFilter::new("ghost", Op::Eq, 1)],
            ..Query::default()
        })
        .is_err()
    );
    let page = db
        .list(&Query {
            sort: vec![Sort::asc("title")],
            limit: Some(2),
            offset: 1,
            ..Query::default()
        })
        .unwrap();
    assert_eq!(titles(&page), ["b", "c"]);
}

#[test]
fn tags_hierarchy_filters_and_cascade() {
    let mut db = db();
    db.create_tag("cooking", None).unwrap();
    db.create_tag("pasta", Some("cooking")).unwrap();
    let a = db
        .create(NewItem::new("task", "carbonara").tag("pasta"))
        .unwrap();
    let b = db
        .create(NewItem::new("task", "eggs").tag("cooking"))
        .unwrap();
    db.create(NewItem::new("task", "laundry").tag("home"))
        .unwrap();
    let by = |f: TagFilter| {
        titles(
            &db.list(&Query {
                tags: f,
                ..Query::default()
            })
            .unwrap(),
        )
        .join(",")
    };
    assert_eq!(
        by(TagFilter {
            all: vec!["cooking".into()],
            ..Default::default()
        }),
        "carbonara,eggs"
    );
    assert_eq!(
        by(TagFilter {
            all: vec!["=cooking".into()],
            ..Default::default()
        }),
        "eggs"
    );
    assert_eq!(
        by(TagFilter {
            exclude: vec!["cooking".into()],
            ..Default::default()
        }),
        "laundry"
    );
    let leaf = TagFilter {
        all: vec!["x".into()],
        ..Default::default()
    };
    let inner = TagFilter {
        blocks: vec![vec![leaf]],
        ..Default::default()
    };
    let nested = TagFilter {
        blocks: vec![vec![inner]],
        ..Default::default()
    };
    assert!(
        db.list(&Query {
            tags: nested,
            ..Query::default()
        })
        .is_err(),
        "nested blocks are rejected"
    );

    let a2 = db.remove_tags(a.id, &["PASTA".into()]).unwrap();
    assert!(a2.tags.is_empty());
    let b2 = db
        .add_tags(b.id, &["quick".into(), "Quick".into()])
        .unwrap();
    assert_eq!(b2.tags, vec!["cooking", "quick"]);
    assert_eq!(
        db.tags()
            .unwrap()
            .iter()
            .find(|t| t.name == "quick")
            .unwrap()
            .count("item"),
        1
    );
    db.delete(b.id).unwrap();
    assert_eq!(
        db.tags()
            .unwrap()
            .iter()
            .find(|t| t.name == "quick")
            .unwrap()
            .count("item"),
        0
    );
}

#[test]
fn reopen_is_idempotent_and_rejects_newer_db() {
    let dir = std::env::temp_dir().join(format!("organiser-engine-test-{}", std::process::id()));
    let path = dir.join("sub").join("t.db");
    {
        let mut db = Db::open(&path, &SCHEMA).unwrap();
        db.create(NewItem::new("task", "kept")).unwrap();
    }
    let db = Db::open(&path, &SCHEMA).unwrap();
    assert_eq!(titles(&db.list(&Query::default()).unwrap()), ["kept"]);
    drop(db);
    rusqlite::Connection::open(&path)
        .unwrap()
        .pragma_update(None, "user_version", 99)
        .unwrap();
    assert!(matches!(Db::open(&path, &SCHEMA), Err(Error::Schema(_))));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn references_must_point_at_the_right_kind_and_are_unlinked_on_delete() {
    let mut db = db();
    let target = db
        .create(NewItem::new("link", "t").field("url", "https://a.b"))
        .unwrap();
    let plain = db.create(NewItem::new("task", "plain")).unwrap();

    assert!(
        db.create(NewItem::new("task", "x").field("link", 999))
            .is_err(),
        "missing target"
    );
    assert!(
        db.create(NewItem::new("task", "x").field("link", plain.id))
            .is_err(),
        "wrong kind"
    );
    let a = db
        .create(NewItem::new("task", "a").field("link", target.id))
        .unwrap();
    assert!(
        db.update(plain.id, ItemPatch::default().field("link", plain.id))
            .is_err()
    );
    assert!(
        db.update(plain.id, ItemPatch::default().field("link", "x"))
            .is_err()
    );

    db.delete(target.id).unwrap();
    let a = db.get(a.id).unwrap();
    assert!(
        !a.fields.contains_key("link"),
        "link removed with its target"
    );
    let q = Query {
        filters: vec![FieldFilter::new("link", Op::NotNull, json!(null))],
        ..Query::default()
    };
    assert!(db.list(&q).unwrap().is_empty());
}

#[test]
fn deleting_a_subtree_unlinks_references_to_every_item_in_it() {
    let mut db = db();
    let root = db
        .create(NewItem::new("link", "root").field("url", "https://a.b"))
        .unwrap();
    let child = db
        .create(
            NewItem::new("link", "child")
                .parent(root.id)
                .field("url", "https://c.d"),
        )
        .unwrap();
    let t = db
        .create(NewItem::new("task", "t").field("link", child.id))
        .unwrap();
    db.delete(root.id).unwrap();
    assert!(!db.get(t.id).unwrap().fields.contains_key("link"));
}

#[test]
fn a_schema_whose_reference_names_no_kind_is_rejected() {
    static BAD: Schema = Schema {
        kinds: &[Kind {
            name: "a",
            fields: &[Field::new("r", FieldType::Reference("ghost"))],
        }],
    };
    assert!(matches!(Db::open_in_memory(&BAD), Err(Error::Schema(_))));
}

// ---- gated fields ------------------------------------------------------------------------

static GATED: Schema = Schema {
    kinds: &[
        Kind {
            name: "list",
            fields: &[Field::new("rated", FieldType::Bool)],
        },
        Kind {
            name: "entry",
            fields: &[
                Field::new("list", FieldType::Reference("list")).required(),
                Field::new("rating", FieldType::Rating).gated("list", "rated"),
            ],
        },
    ],
};

fn gated_db() -> (Db, i64, i64) {
    let mut db = Db::open_in_memory(&GATED).unwrap();
    let rated = db
        .create(NewItem::new("list", "Movies").field("rated", true))
        .unwrap()
        .id;
    let plain = db
        .create(NewItem::new("list", "Animals").field("rated", false))
        .unwrap()
        .id;
    (db, rated, plain)
}

#[test]
fn a_gated_field_is_allowed_only_while_its_flag_is_on() {
    let (mut db, rated, plain) = gated_db();
    let ok = db
        .create(
            NewItem::new("entry", "Alien")
                .field("list", rated)
                .field("rating", 5),
        )
        .unwrap();
    assert_eq!(ok.fields["rating"], 5);
    assert!(
        db.create(
            NewItem::new("entry", "Cat")
                .field("list", plain)
                .field("rating", 4)
        )
        .is_err()
    );
    // Without a rating both lists accept the entry.
    db.create(NewItem::new("entry", "Cat").field("list", plain))
        .unwrap();
    // A list that never said anything about `rated` counts as off.
    let unset = db.create(NewItem::new("list", "Cars")).unwrap().id;
    assert!(
        db.create(
            NewItem::new("entry", "Mini")
                .field("list", unset)
                .field("rating", 3)
        )
        .is_err()
    );
    // Updates are checked as well.
    let cat = db
        .create(NewItem::new("entry", "Dog").field("list", plain))
        .unwrap();
    assert!(
        db.update(cat.id, ItemPatch::default().field("rating", 2))
            .is_err()
    );
}

#[test]
fn switching_a_flag_off_removes_what_it_allowed() {
    let (mut db, rated, _) = gated_db();
    let a = db
        .create(
            NewItem::new("entry", "A")
                .field("list", rated)
                .field("rating", 5),
        )
        .unwrap();
    let b = db
        .create(NewItem::new("entry", "B").field("list", rated))
        .unwrap();
    db.update(rated, ItemPatch::default().field("rated", false))
        .unwrap();
    assert!(
        !db.get(a.id).unwrap().fields.contains_key("rating"),
        "the rating went with the switch"
    );
    assert_eq!(
        db.get(a.id).unwrap().fields["list"],
        rated,
        "the rest of the entry is untouched"
    );
    assert_eq!(db.get(b.id).unwrap().title, "B");
    // Switching it back on does not bring ratings back, but new ones are allowed.
    db.update(rated, ItemPatch::default().field("rated", true))
        .unwrap();
    assert!(!db.get(a.id).unwrap().fields.contains_key("rating"));
    db.update(a.id, ItemPatch::default().field("rating", 4))
        .unwrap();
}

#[test]
fn switching_one_list_off_leaves_other_lists_alone() {
    let (mut db, rated, _) = gated_db();
    let other = db
        .create(NewItem::new("list", "Series").field("rated", true))
        .unwrap()
        .id;
    let keep = db
        .create(
            NewItem::new("entry", "Keep")
                .field("list", other)
                .field("rating", 5),
        )
        .unwrap();
    db.create(
        NewItem::new("entry", "Drop")
            .field("list", rated)
            .field("rating", 5),
    )
    .unwrap();
    db.update(rated, ItemPatch::default().field("rated", false))
        .unwrap();
    assert_eq!(db.get(keep.id).unwrap().fields["rating"], 5);
}

#[test]
fn a_badly_gated_schema_is_rejected() {
    static NO_VIA: Schema = Schema {
        kinds: &[Kind {
            name: "e",
            fields: &[Field::new("rating", FieldType::Rating).gated("list", "rated")],
        }],
    };
    static NOT_BOOL: Schema = Schema {
        kinds: &[
            Kind {
                name: "list",
                fields: &[Field::new("rated", FieldType::Text)],
            },
            Kind {
                name: "e",
                fields: &[
                    Field::new("list", FieldType::Reference("list")),
                    Field::new("rating", FieldType::Rating).gated("list", "rated"),
                ],
            },
        ],
    };
    assert!(matches!(Db::open_in_memory(&NO_VIA), Err(Error::Schema(_))));
    assert!(matches!(
        Db::open_in_memory(&NOT_BOOL),
        Err(Error::Schema(_))
    ));
}

#[test]
fn a_field_can_be_gated_through_the_parent() {
    static BY_PARENT: Schema = Schema {
        kinds: &[
            Kind {
                name: "list",
                fields: &[Field::new("rated", FieldType::Bool)],
            },
            Kind {
                name: "entry",
                fields: &[Field::new("rating", FieldType::Rating).gated_by_parent("list", "rated")],
            },
        ],
    };
    let mut db = Db::open_in_memory(&BY_PARENT).unwrap();
    let on = db
        .create(NewItem::new("list", "On").field("rated", true))
        .unwrap()
        .id;
    let off = db.create(NewItem::new("list", "Off")).unwrap().id;
    let e = db
        .create(NewItem::new("entry", "A").parent(on).field("rating", 5))
        .unwrap();
    assert!(
        db.create(NewItem::new("entry", "B").parent(off).field("rating", 5))
            .is_err()
    );
    assert!(
        db.create(NewItem::new("entry", "C").field("rating", 5))
            .is_err(),
        "no parent, no gate"
    );
    db.create(NewItem::new("entry", "D").parent(off)).unwrap();
    db.update(on, ItemPatch::default().field("rated", false))
        .unwrap();
    assert!(!db.get(e.id).unwrap().fields.contains_key("rating"));
    assert_eq!(db.delete(on).unwrap(), 2, "the entries go with their list");
}
