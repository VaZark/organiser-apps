use notes_schema::SCHEMA;
use notes_schema::color::Color;
use notes_schema::lifecycle::{
    NewNote, create, edit, empty_trash, purge, purge_older_than, restore, set_archived, set_pinned,
    set_tags, trash,
};
use notes_schema::note::{Edit, Note};
use notes_schema::views::{Filter, Scope, notes};
use organiser_engine::Db;

fn db() -> Db {
    Db::open_in_memory(&SCHEMA).unwrap()
}

fn titles(v: &[Note]) -> Vec<&str> {
    v.iter().map(|n| n.title.as_str()).collect()
}

fn in_scope(db: &Db, scope: Scope) -> Vec<Note> {
    notes(
        db,
        &Filter {
            scope,
            ..Filter::default()
        },
    )
    .unwrap()
}

#[test]
fn new_notes_keep_body_custom_color_and_tags() {
    let mut db = db();
    let n = create(
        &mut db,
        &NewNote {
            title: "Ideas".into(),
            body: "- one".into(),
            color: Some(Color::rgb(10, 200, 30)),
            tags: vec!["work".into()],
        },
    )
    .unwrap();
    assert_eq!(n.body, "- one");
    assert_eq!(n.color, Some(Color::rgb(10, 200, 30)));
    assert_eq!(n.tags, ["work"]);
    assert!(!n.pinned && !n.archived && !n.is_trashed());
}

#[test]
fn empty_title_is_rejected() {
    assert!(create(&mut db(), &NewNote::new("  ")).is_err());
}

#[test]
fn edit_changes_color_and_clears_it() {
    let mut db = db();
    let n = create(&mut db, &NewNote::new("a")).unwrap();
    let n = edit(
        &mut db,
        n.id,
        &Edit {
            color: Some(Some(Color::rgb(1, 2, 3))),
            ..Edit::default()
        },
    )
    .unwrap();
    assert_eq!(n.color, Some(Color::rgb(1, 2, 3)));
    let n = edit(
        &mut db,
        n.id,
        &Edit {
            color: Some(None),
            ..Edit::default()
        },
    )
    .unwrap();
    assert_eq!(n.color, None);
}

#[test]
fn an_unreadable_stored_color_falls_back_to_none() {
    let mut db = db();
    let n = create(&mut db, &NewNote::new("a")).unwrap();
    db.update(
        n.id,
        organiser_engine::ItemPatch::default().field("color", "not-a-color"),
    )
    .unwrap();
    assert_eq!(Note::from(&db.get(n.id).unwrap()).color, None);
}

#[test]
fn pinned_notes_come_first_then_newest() {
    let mut db = db();
    let a = create(&mut db, &NewNote::new("a")).unwrap();
    create(&mut db, &NewNote::new("b")).unwrap();
    set_pinned(&mut db, a.id, true).unwrap();
    assert_eq!(titles(&in_scope(&db, Scope::Notes)), ["a", "b"]);
    set_pinned(&mut db, a.id, false).unwrap();
    assert!(!Note::from(&db.get(a.id).unwrap()).pinned);
}

#[test]
fn archive_moves_a_note_out_of_notes_and_back() {
    let mut db = db();
    let a = create(&mut db, &NewNote::new("a")).unwrap();
    set_archived(&mut db, a.id, true).unwrap();
    assert!(in_scope(&db, Scope::Notes).is_empty());
    assert_eq!(titles(&in_scope(&db, Scope::Archive)), ["a"]);
    set_archived(&mut db, a.id, false).unwrap();
    assert_eq!(titles(&in_scope(&db, Scope::Notes)), ["a"]);
}

#[test]
fn trash_hides_unpins_and_restore_brings_back() {
    let mut db = db();
    let a = create(&mut db, &NewNote::new("a")).unwrap();
    set_pinned(&mut db, a.id, true).unwrap();
    let t = trash(&mut db, a.id, 100).unwrap();
    assert_eq!((t.trashed, t.pinned), (Some(100), false));
    assert!(in_scope(&db, Scope::Notes).is_empty());
    assert_eq!(titles(&in_scope(&db, Scope::Trash)), ["a"]);
    restore(&mut db, a.id).unwrap();
    assert_eq!(titles(&in_scope(&db, Scope::Notes)), ["a"]);
    assert!(in_scope(&db, Scope::Trash).is_empty());
}

#[test]
fn trashed_archived_notes_show_only_in_trash() {
    let mut db = db();
    let a = create(&mut db, &NewNote::new("a")).unwrap();
    set_archived(&mut db, a.id, true).unwrap();
    trash(&mut db, a.id, 1).unwrap();
    assert!(in_scope(&db, Scope::Archive).is_empty());
    assert_eq!(titles(&in_scope(&db, Scope::Trash)), ["a"]);
}

#[test]
fn purge_needs_the_trash_first() {
    let mut db = db();
    let a = create(&mut db, &NewNote::new("a")).unwrap();
    let err = purge(&mut db, a.id).unwrap_err().to_string();
    assert!(err.contains("trash it first"), "{err}");
    trash(&mut db, a.id, 1).unwrap();
    purge(&mut db, a.id).unwrap();
    assert!(db.get(a.id).is_err());
}

#[test]
fn purge_older_than_keeps_recent_and_empty_trash_takes_all() {
    let mut db = db();
    let old = create(&mut db, &NewNote::new("old")).unwrap();
    let new = create(&mut db, &NewNote::new("new")).unwrap();
    let live = create(&mut db, &NewNote::new("live")).unwrap();
    trash(&mut db, old.id, 10).unwrap();
    trash(&mut db, new.id, 90).unwrap();
    assert_eq!(purge_older_than(&mut db, 50).unwrap(), 1);
    assert_eq!(titles(&in_scope(&db, Scope::Trash)), ["new"]);
    assert_eq!(empty_trash(&mut db).unwrap(), 1);
    assert_eq!(titles(&in_scope(&db, Scope::Notes)), ["live"]);
    assert!(db.get(live.id).is_ok());
}

#[test]
fn search_matches_title_and_body_and_can_include_the_archive() {
    let mut db = db();
    create(
        &mut db,
        &NewNote {
            title: "Groceries".into(),
            body: "milk".into(),
            ..NewNote::default()
        },
    )
    .unwrap();
    let b = create(
        &mut db,
        &NewNote {
            title: "Trip".into(),
            body: "milk run".into(),
            ..NewNote::default()
        },
    )
    .unwrap();
    set_archived(&mut db, b.id, true).unwrap();
    let f = |include_archived| Filter {
        text: Some("milk".into()),
        include_archived,
        ..Filter::default()
    };
    assert_eq!(titles(&notes(&db, &f(false)).unwrap()), ["Groceries"]);
    assert_eq!(notes(&db, &f(true)).unwrap().len(), 2);
}

#[test]
fn tag_filter_narrows_the_list() {
    let mut db = db();
    let a = create(&mut db, &NewNote::new("a")).unwrap();
    create(&mut db, &NewNote::new("b")).unwrap();
    set_tags(&mut db, a.id, &["work".into()]).unwrap();
    let f = Filter {
        tags: organiser_engine::TagFilter {
            all: vec!["work".into()],
            ..Default::default()
        },
        ..Filter::default()
    };
    assert_eq!(titles(&notes(&db, &f).unwrap()), ["a"]);
}

#[test]
fn notes_expire_from_the_trash_after_the_retention_period() {
    use notes_schema::lifecycle::{TRASH_DAYS, purge_expired};
    let day = 86_400;
    let now = 100 * day;
    let mut db = db();
    let old = create(&mut db, &NewNote::new("old")).unwrap();
    let fresh = create(&mut db, &NewNote::new("fresh")).unwrap();
    trash(&mut db, old.id, now - TRASH_DAYS * day).unwrap();
    trash(&mut db, fresh.id, now - (TRASH_DAYS - 1) * day).unwrap();
    assert_eq!(purge_expired(&mut db, now, TRASH_DAYS as u32).unwrap(), 1);
    assert_eq!(titles(&in_scope(&db, Scope::Trash)), ["fresh"]);
}

#[test]
fn schema_filters_narrow_the_list_and_the_trash_date_is_not_one_of_them() {
    use organiser_engine::{Filters, filterable};
    let kind = &SCHEMA.kinds[0];
    let offered: Vec<_> = filterable(kind).iter().map(|f| f.name).collect();
    assert_eq!(
        offered,
        ["pinned"],
        "the trash date is bookkeeping, not a filter"
    );
    let mut db = db();
    let a = create(&mut db, &NewNote::new("a")).unwrap();
    create(&mut db, &NewNote::new("b")).unwrap();
    set_pinned(&mut db, a.id, true).unwrap();
    let mut fields = Filters::default();
    fields.toggle("pinned", serde_json::json!(true));
    let f = Filter {
        fields,
        ..Filter::default()
    };
    assert_eq!(titles(&notes(&db, &f).unwrap()), ["a"]);
}

#[test]
fn tag_groups_are_alternatives() {
    use organiser_engine::TagFilter;
    let mut db = db();
    let a = create(&mut db, &NewNote::new("a")).unwrap();
    let b = create(&mut db, &NewNote::new("b")).unwrap();
    let c = create(&mut db, &NewNote::new("c")).unwrap();
    set_tags(&mut db, a.id, &["work".into(), "urgent".into()]).unwrap();
    set_tags(&mut db, b.id, &["work".into()]).unwrap();
    set_tags(&mut db, c.id, &["home".into()]).unwrap();
    let group = |tags: &[&str]| TagFilter {
        all: tags.iter().map(|t| t.to_string()).collect(),
        ..TagFilter::default()
    };
    // (work and urgent) or home: `b` has only work, so it is out.
    let f = Filter {
        tags: TagFilter {
            blocks: vec![vec![group(&["work", "urgent"]), group(&["home"])]],
            ..TagFilter::default()
        },
        ..Filter::default()
    };
    let found = notes(&db, &f).unwrap();
    let mut got = titles(&found);
    got.sort();
    assert_eq!(got, ["a", "c"]);
}
