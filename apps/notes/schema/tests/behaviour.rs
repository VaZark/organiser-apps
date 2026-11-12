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
            ..NewNote::default()
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
    let kind = SCHEMA.kinds.iter().find(|k| k.name == "note").unwrap();
    let offered: Vec<_> = filterable(kind).iter().map(|f| f.name).collect();
    assert_eq!(
        offered,
        ["favorite", "rating", "pinned"],
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

// ---- folders, favourites and ratings (merged in from the Faves app) ------------------------

mod folders {
    use super::*;
    use notes_schema::folder::FolderEdit;
    use notes_schema::lifecycle::{
        create_folder, delete_folder, delete_folder_and_notes, edit_folder, ensure_default_folder,
        move_note, set_favorite,
    };
    use notes_schema::views::{Sort, folders};

    fn note_in(db: &mut Db, folder: Option<i64>, title: &str) -> Note {
        create(
            db,
            &NewNote {
                title: title.into(),
                folder,
                ..NewNote::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn a_note_with_no_folder_is_in_the_default_one_and_can_be_filed_and_unfiled() {
        let mut db = db();
        let loose = note_in(&mut db, None, "Loose");
        let home = ensure_default_folder(&mut db).unwrap();
        assert_eq!(loose.folder_id, Some(home), "no folder is the default folder");
        let f = create_folder(&mut db, "Work", false).unwrap();
        assert_eq!(move_note(&mut db, loose.id, f.id).unwrap().folder_id, Some(f.id));
        assert_eq!(titles(&in_scope(&db, Scope::Folder(f.id))), ["Loose"]);
        // Out of a folder is back into the default one.
        assert_eq!(move_note(&mut db, loose.id, None).unwrap().folder_id, Some(home));
        assert!(in_scope(&db, Scope::Folder(f.id)).is_empty());
        // Folders are not notes: they never show among them.
        assert_eq!(titles(&in_scope(&db, Scope::Notes)), ["Loose"]);
    }

    #[test]
    fn folders_count_their_live_notes_and_favourites() {
        let mut db = db();
        let f = create_folder(&mut db, "Work", false).unwrap();
        let a = note_in(&mut db, Some(f.id), "A");
        let b = note_in(&mut db, Some(f.id), "B");
        note_in(&mut db, None, "Unfiled");
        set_favorite(&mut db, a.id, true).unwrap();
        // Archived and trashed notes are not counted.
        trash(&mut db, b.id, 1).unwrap();
        let all = folders(&db).unwrap();
        let s = all.iter().find(|s| s.folder.id == f.id).unwrap();
        assert_eq!((s.notes, s.favorites), (1, 1));
        // The default folder is last, with the note in no other.
        let last = all.last().unwrap();
        assert!(last.folder.is_default);
        assert_eq!((last.notes, last.favorites), (1, 0));
        assert_eq!(titles(&in_scope(&db, Scope::Favorites)), ["A"]);
    }

    #[test]
    fn deleting_a_folder_keeps_or_deletes_its_notes() {
        let mut db = db();
        let keep = create_folder(&mut db, "Keep", false).unwrap();
        let gone = create_folder(&mut db, "Gone", false).unwrap();
        note_in(&mut db, Some(keep.id), "stays");
        note_in(&mut db, Some(gone.id), "goes");
        delete_folder(&mut db, keep.id).unwrap();
        assert_eq!(in_scope(&db, Scope::Notes).len(), 2);
        let home = ensure_default_folder(&mut db).unwrap();
        let stays = in_scope(&db, Scope::Notes).into_iter().find(|n| n.title == "stays").unwrap();
        assert_eq!(stays.folder_id, Some(home), "its notes are in the default folder");
        assert_eq!(delete_folder_and_notes(&mut db, gone.id).unwrap(), 1);
        assert_eq!(titles(&in_scope(&db, Scope::Notes)), ["stays"]);
    }

    #[test]
    fn a_rating_is_set_only_where_the_folder_offers_it_and_is_hidden_when_it_stops() {
        let mut db = db();
        let f = create_folder(&mut db, "Movies", true).unwrap();
        let n = note_in(&mut db, Some(f.id), "Alien");
        let rate = |r| Edit {
            rating: Some(r),
            ..Edit::default()
        };
        // Not in a folder that rates: refused. In one: taken.
        let loose = note_in(&mut db, None, "Loose");
        assert!(edit(&mut db, loose.id, &rate(Some(4))).is_err());
        assert_eq!(edit(&mut db, n.id, &rate(Some(5))).unwrap().rating, Some(5));

        let five = Filter {
            fields: {
                let mut x = organiser_engine::Filters::default();
                x.set_range("rating", Some(serde_json::json!(5)), None);
                x
            },
            ..Filter::default()
        };
        assert_eq!(titles(&notes(&db, &five).unwrap()), ["Alien"]);

        // Switched off: kept, but not shown, not matched, not sorted by.
        edit_folder(
            &mut db,
            f.id,
            &FolderEdit {
                rated: Some(false),
                ..FolderEdit::default()
            },
        )
        .unwrap();
        assert_eq!(db.get(n.id).map(|i| Note::from(&i)).unwrap().rating, Some(5));
        assert_eq!(in_scope(&db, Scope::Notes)[0].rating, None);
        assert!(notes(&db, &five).unwrap().is_empty());
        // Back on: it is there again.
        edit_folder(
            &mut db,
            f.id,
            &FolderEdit {
                rated: Some(true),
                ..FolderEdit::default()
            },
        )
        .unwrap();
        assert_eq!(in_scope(&db, Scope::Notes)[0].rating, Some(5));
        // Moving it out keeps the stored rating, hidden.
        move_note(&mut db, n.id, None).unwrap();
        assert_eq!(db.get(n.id).map(|i| Note::from(&i)).unwrap().rating, Some(5));
        assert!(in_scope(&db, Scope::Notes).iter().all(|n| n.rating.is_none()));
    }

    #[test]
    fn sorting_by_rating_puts_the_best_first_and_pinned_ahead() {
        let mut db = db();
        let f = create_folder(&mut db, "Movies", true).unwrap();
        let mk = |db: &mut Db, title: &str, r: Option<u8>| {
            create(
                db,
                &NewNote {
                    title: title.into(),
                    folder: Some(f.id),
                    rating: r,
                    ..NewNote::default()
                },
            )
            .unwrap()
        };
        mk(&mut db, "Four", Some(4));
        mk(&mut db, "Unrated", None);
        let five = mk(&mut db, "Five", Some(5));
        let by = |db: &Db| {
            notes(
                db,
                &Filter {
                    sort: Sort::Rating,
                    ..Filter::default()
                },
            )
            .unwrap()
        };
        assert_eq!(titles(&by(&db)), ["Five", "Four", "Unrated"]);
        let four = by(&db)[1].id;
        set_pinned(&mut db, four, true).unwrap();
        assert_eq!(titles(&by(&db)), ["Four", "Five", "Unrated"], "pinned first");
        let _ = five;
    }

    #[test]
    fn a_folder_has_a_colour_and_trashed_notes_keep_their_folder() {
        let mut db = db();
        let f = create_folder(&mut db, "Work", false).unwrap();
        let c = edit_folder(
            &mut db,
            f.id,
            &FolderEdit {
                color: Some(Some(Color::rgb(1, 2, 3))),
                ..FolderEdit::default()
            },
        )
        .unwrap();
        assert_eq!(c.color, Some(Color::rgb(1, 2, 3)));
        let n = note_in(&mut db, Some(f.id), "A");
        trash(&mut db, n.id, 5).unwrap();
        // In the trash it is still in its folder, and back where it was after a restore.
        assert_eq!(in_scope(&db, Scope::Trash)[0].folder_id, Some(f.id));
        assert_eq!(restore(&mut db, n.id).unwrap().folder_id, Some(f.id));
    }
}

mod sections {
    use super::*;
    use notes_schema::lifecycle::{
        create_folder, create_section, delete_folder, delete_section, edit_section, move_note,
        set_section,
    };
    use notes_schema::section::SectionEdit;
    use notes_schema::views::sections;

    fn filed(db: &mut Db, folder: Option<i64>, section: Option<i64>, title: &str) -> Note {
        create(
            db,
            &NewNote {
                title: title.into(),
                folder,
                section,
                ..NewNote::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn a_note_is_in_a_section_of_its_own_folder_only() {
        let mut db = db();
        let a = create_folder(&mut db, "A", false).unwrap();
        let b = create_folder(&mut db, "B", false).unwrap();
        let sa = create_section(&mut db, a.id, "SA").unwrap();
        let sb = create_section(&mut db, b.id, "SB").unwrap();
        let n = filed(&mut db, Some(a.id), Some(sa.id), "n");
        assert_eq!(n.section, Some(sa.id));
        // Another folder's section, or one with no folder, is refused.
        assert!(set_section(&mut db, n.id, Some(sb.id)).is_err());
        let loose = filed(&mut db, None, None, "loose");
        assert!(set_section(&mut db, loose.id, Some(sa.id)).is_err());
        assert!(
            create(
                &mut db,
                &NewNote {
                    title: "x".into(),
                    folder: Some(a.id),
                    section: Some(sb.id),
                    ..NewNote::default()
                }
            )
            .is_err()
        );
        assert_eq!(set_section(&mut db, n.id, None).unwrap().section, None);
    }

    #[test]
    fn moving_to_another_folder_leaves_the_section_behind() {
        let mut db = db();
        let a = create_folder(&mut db, "A", false).unwrap();
        let b = create_folder(&mut db, "B", false).unwrap();
        let sa = create_section(&mut db, a.id, "SA").unwrap();
        let n = filed(&mut db, Some(a.id), Some(sa.id), "n");
        let moved = move_note(&mut db, n.id, b.id).unwrap();
        assert_eq!((moved.folder_id, moved.section), (Some(b.id), None));
    }

    #[test]
    fn sections_count_their_live_notes_and_are_not_notes() {
        let mut db = db();
        let f = create_folder(&mut db, "F", false).unwrap();
        let s1 = create_section(&mut db, f.id, "One").unwrap();
        let s2 = create_section(&mut db, f.id, "Two").unwrap();
        filed(&mut db, Some(f.id), Some(s1.id), "a");
        let b = filed(&mut db, Some(f.id), Some(s1.id), "b");
        filed(&mut db, Some(f.id), None, "c");
        trash(&mut db, b.id, 1).unwrap();
        let counts: Vec<(String, usize)> = sections(&db, f.id)
            .unwrap()
            .into_iter()
            .map(|s| (s.section.title, s.notes))
            .collect();
        assert_eq!(counts, [("One".into(), 1), ("Two".into(), 0)]);
        // Neither the folder nor its sections show among the notes.
        let live = in_scope(&db, Scope::Folder(f.id));
        let mut shown = titles(&live);
        shown.sort();
        assert_eq!(shown, ["a", "c"]);
        let _ = s2;
    }

    #[test]
    fn a_section_has_a_colour_and_a_description_and_deleting_it_keeps_its_notes() {
        let mut db = db();
        let f = create_folder(&mut db, "F", false).unwrap();
        let s = create_section(&mut db, f.id, "One").unwrap();
        let e = edit_section(
            &mut db,
            s.id,
            &SectionEdit {
                notes: Some("why".into()),
                color: Some(Some(Color::rgb(1, 2, 3))),
                ..SectionEdit::default()
            },
        )
        .unwrap();
        assert_eq!((e.notes.as_str(), e.color), ("why", Some(Color::rgb(1, 2, 3))));
        let n = filed(&mut db, Some(f.id), Some(s.id), "n");
        delete_section(&mut db, s.id).unwrap();
        let n = in_scope(&db, Scope::Folder(f.id)).into_iter().find(|x| x.id == n.id).unwrap();
        assert_eq!((n.folder_id, n.section), (Some(f.id), None));
    }

    #[test]
    fn deleting_a_folder_takes_its_sections_and_keeps_its_notes() {
        let mut db = db();
        let f = create_folder(&mut db, "F", false).unwrap();
        let s = create_section(&mut db, f.id, "One").unwrap();
        filed(&mut db, Some(f.id), Some(s.id), "n");
        delete_folder(&mut db, f.id).unwrap();
        assert!(db.get(s.id).is_err(), "the section went with the folder");
        let left = in_scope(&db, Scope::Notes);
        assert_eq!(titles(&left), ["n"]);
        let home = notes_schema::lifecycle::ensure_default_folder(&mut db).unwrap();
        assert_eq!((left[0].folder_id, left[0].section), (Some(home), None));
    }
}

#[test]
fn the_default_folder_is_made_once_adopts_loose_notes_and_cannot_be_deleted() {
    use notes_schema::lifecycle::{delete_folder, delete_folder_and_notes, ensure_default_folder};
    use notes_schema::views::folders;
    let mut db = db();
    // A note made by a tool that knows nothing of folders: it has no parent.
    let loose = db
        .create(organiser_engine::NewItem::new("note", "loose"))
        .unwrap();
    let home = ensure_default_folder(&mut db).unwrap();
    assert_eq!(ensure_default_folder(&mut db).unwrap(), home, "made once");
    let all = folders(&db).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!((all[0].folder.title.as_str(), all[0].folder.is_default), ("Unassigned", true));
    assert_eq!(
        Note::from(&db.get(loose.id).unwrap()).folder_id,
        Some(home),
        "adopted"
    );
    assert!(delete_folder(&mut db, home).is_err());
    assert!(delete_folder_and_notes(&mut db, home).is_err());
    assert_eq!(folders(&db).unwrap().len(), 1, "still there");
}

#[test]
fn a_section_can_be_moved_to_another_place() {
    use notes_schema::lifecycle::{create_folder, create_section, move_section};
    use notes_schema::views::sections;
    let mut db = db();
    let f = create_folder(&mut db, "F", false).unwrap();
    let a = create_section(&mut db, f.id, "A").unwrap();
    let b = create_section(&mut db, f.id, "B").unwrap();
    let c = create_section(&mut db, f.id, "C").unwrap();
    let order = |db: &Db| -> Vec<String> {
        sections(db, f.id).unwrap().into_iter().map(|s| s.section.title).collect()
    };
    assert_eq!(order(&db), ["A", "B", "C"]);
    move_section(&mut db, c.id, 0).unwrap();
    assert_eq!(order(&db), ["C", "A", "B"]);
    move_section(&mut db, a.id, 2).unwrap();
    assert_eq!(order(&db), ["C", "B", "A"]);
    move_section(&mut db, c.id, 99).unwrap();
    assert_eq!(order(&db), ["B", "A", "C"]);
    let _ = b;
}

#[test]
fn a_folder_can_be_moved_before_another_and_the_default_stays_last() {
    use notes_schema::lifecycle::{create_folder, move_folder};
    use notes_schema::views::folders;
    let mut db = db();
    let a = create_folder(&mut db, "A", false).unwrap();
    let b = create_folder(&mut db, "B", false).unwrap();
    let c = create_folder(&mut db, "C", false).unwrap();
    let order = |db: &Db| -> Vec<String> {
        folders(db).unwrap().into_iter().map(|s| s.folder.title).collect()
    };
    let user: Vec<String> = order(&db).into_iter().filter(|t| ["A", "B", "C"].contains(&t.as_str())).collect();
    assert_eq!(user, ["A", "B", "C"]);
    move_folder(&mut db, c.id, Some(a.id)).unwrap();
    let now = order(&db);
    assert_eq!(&now[..3], ["C", "A", "B"]);
    move_folder(&mut db, c.id, None).unwrap();
    assert_eq!(&order(&db)[..3], ["A", "B", "C"]);
    let _ = (a, b);
}
