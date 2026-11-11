use chrono::NaiveDate;
use organiser_engine::{Db, TagFilter};
use todo_schema::SCHEMA;
use todo_schema::lifecycle::{
    NewTask, create, create_epic, delete_epic, edit, set_epic_status, set_status, set_tags,
};
use todo_schema::priority::Priority;
use todo_schema::status::Status;
use todo_schema::statuses::{self, StatusEdit, ensure_defaults};
use todo_schema::task::{Edit, Task};
use todo_schema::views::{Filter, Scope, epics, forest, subtasks, tasks};

fn d(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

fn titles(v: &[Task]) -> Vec<&str> {
    v.iter().map(|t| t.title.as_str()).collect()
}

/// The statuses a new database starts with, in the order they are created.
const OPEN: i64 = 1;
const DOING: i64 = 2;
const DONE: i64 = 3;
const WONT: i64 = 4;

fn db() -> Db {
    let mut db = Db::open_in_memory(&SCHEMA).unwrap();
    ensure_defaults(&mut db).unwrap();
    db
}

fn task(db: &mut Db, title: &str) -> Task {
    create(db, &NewTask::new(title)).unwrap()
}

const TODAY: &str = "2026-11-10";

fn all(db: &Db, f: &Filter) -> Vec<Task> {
    tasks(db, f, d(TODAY)).unwrap()
}

#[test]
fn new_tasks_default_to_open_medium_and_keep_dates_and_tags() {
    let mut db = db();
    let t = create(
        &mut db,
        &NewTask {
            title: "Write report".into(),
            start: Some(d("2026-11-01")),
            due: Some(d("2026-11-20")),
            tags: vec!["work".into()],
            ..NewTask::default()
        },
    )
    .unwrap();
    assert_eq!((t.status, t.priority), (Some(OPEN), Priority::Medium));
    assert_eq!(
        (t.start, t.due),
        (Some(d("2026-11-01")), Some(d("2026-11-20")))
    );
    assert_eq!(t.tags, ["work"]);
}

#[test]
fn closed_tasks_are_hidden_unless_asked_for() {
    let mut db = db();
    let a = task(&mut db, "a");
    task(&mut db, "b");
    set_status(&mut db, a.id, DONE).unwrap();
    assert_eq!(titles(&all(&db, &Filter::default())), ["b"]);
    let f = Filter {
        show_closed: true,
        ..Filter::default()
    };
    assert_eq!(titles(&all(&db, &f)), ["a", "b"]);
}

#[test]
fn in_progress_counts_as_open_and_does_not_cascade() {
    let mut db = db();
    let a = task(&mut db, "a");
    let b = create(
        &mut db,
        &NewTask {
            parent: Some(a.id),
            ..NewTask::new("b")
        },
    )
    .unwrap();
    set_status(&mut db, a.id, DOING).unwrap();
    let open = all(&db, &Filter::default());
    assert_eq!(
        open.iter().map(|t| t.status).collect::<Vec<_>>(),
        [Some(DOING), Some(OPEN)]
    );
    assert_eq!(subtasks(&db, a.id).unwrap()[0].id, b.id);
}

#[test]
fn closing_cascades_down_and_reopening_cascades_up() {
    let mut db = db();
    let a = task(&mut db, "a");
    let b = create(
        &mut db,
        &NewTask {
            parent: Some(a.id),
            ..NewTask::new("b")
        },
    )
    .unwrap();
    let c = create(
        &mut db,
        &NewTask {
            parent: Some(b.id),
            ..NewTask::new("c")
        },
    )
    .unwrap();
    assert_eq!(set_status(&mut db, a.id, DONE).unwrap().len(), 3);
    let closed = Filter {
        show_closed: true,
        ..Filter::default()
    };
    assert!(all(&db, &Filter::default()).is_empty());
    assert_eq!(all(&db, &closed).len(), 3);

    set_status(&mut db, c.id, OPEN).unwrap();
    assert_eq!(
        titles(&all(&db, &Filter::default())),
        ["a", "b", "c"],
        "ancestors reopen with it"
    );
    assert!(
        set_status(&mut db, c.id, OPEN).unwrap().is_empty(),
        "no-op changes nothing"
    );
}

#[test]
fn dismissing_a_parent_leaves_already_done_children_done() {
    let mut db = db();
    let a = task(&mut db, "a");
    let b = create(
        &mut db,
        &NewTask {
            parent: Some(a.id),
            ..NewTask::new("b")
        },
    )
    .unwrap();
    set_status(&mut db, b.id, DONE).unwrap();
    set_status(&mut db, a.id, WONT).unwrap();
    let f = Filter {
        show_closed: true,
        ..Filter::default()
    };
    let got = all(&db, &f);
    let st = |n: &str| got.iter().find(|t| t.title == n).unwrap().status;
    assert_eq!((st("a"), st("b")), (Some(WONT), Some(DONE)));
}

#[test]
fn current_tasks_are_scheduled_tasks_whose_span_includes_today_soonest_deadline_first() {
    let mut db = db();
    // (title, start, due)
    let cases = [
        ("undated", None, None),
        ("started, open ended", Some("2026-11-01"), None),
        ("due later", None, Some("2026-11-20")),
        ("due today", None, Some(TODAY)),
        ("starts today", Some(TODAY), None),
        ("in a range", Some("2026-11-05"), Some("2026-11-15")),
        ("overdue", None, Some("2026-11-09")),
        ("not started", Some("2026-11-11"), None),
        ("range in the past", Some("2026-10-01"), Some("2026-10-31")),
        (
            "range in the future",
            Some("2026-12-01"),
            Some("2026-12-31"),
        ),
    ];
    for (title, start, due) in cases {
        create(
            &mut db,
            &NewTask {
                start: start.map(d),
                due: due.map(d),
                ..NewTask::new(title)
            },
        )
        .unwrap();
    }
    let current = all(
        &db,
        &Filter {
            scope: Scope::Current,
            ..Filter::default()
        },
    );
    assert_eq!(
        titles(&current),
        [
            "due today",
            "in a range",
            "due later",
            "started, open ended",
            "starts today"
        ],
        "undated, overdue and not-yet-started tasks are left out"
    );
    assert_eq!(all(&db, &Filter::default()).len(), 10);
}

#[test]
fn current_lists_each_task_alone_without_pulling_in_its_family() {
    let mut db = db();
    let parent = create(&mut db, &NewTask::new("parent, undated")).unwrap();
    let dated = create(
        &mut db,
        &NewTask {
            parent: Some(parent.id),
            due: Some(d("2026-11-12")),
            ..NewTask::new("dated subtask")
        },
    )
    .unwrap();
    create(
        &mut db,
        &NewTask {
            parent: Some(parent.id),
            ..NewTask::new("undated sibling")
        },
    )
    .unwrap();
    let current = all(
        &db,
        &Filter {
            scope: Scope::Current,
            ..Filter::default()
        },
    );
    assert_eq!(titles(&current), ["dated subtask"]);
    assert_eq!(
        current[0].parent_id,
        Some(parent.id),
        "it still knows its parent"
    );
    assert_eq!(current[0].id, dated.id);
}

#[test]
fn epics_group_tasks_and_report_progress() {
    let mut db = db();
    let e = create_epic(&mut db, "Holiday").unwrap();
    let other = create_epic(&mut db, "Work").unwrap();
    let a = create(
        &mut db,
        &NewTask {
            epic: Some(e.id),
            ..NewTask::new("flights")
        },
    )
    .unwrap();
    create(
        &mut db,
        &NewTask {
            epic: Some(e.id),
            ..NewTask::new("hotel")
        },
    )
    .unwrap();
    create(
        &mut db,
        &NewTask {
            epic: Some(other.id),
            ..NewTask::new("report")
        },
    )
    .unwrap();
    task(&mut db, "loose");

    let f = Filter {
        epic: Some(e.id),
        ..Filter::default()
    };
    assert_eq!(titles(&all(&db, &f)), ["flights", "hotel"]);
    set_status(&mut db, a.id, DONE).unwrap();
    let list = epics(&db, false).unwrap();
    let holiday = list.iter().find(|s| s.epic.id == e.id).unwrap();
    assert_eq!(
        (holiday.open, holiday.total, holiday.progress()),
        (1, 2, 0.5)
    );

    set_epic_status(&mut db, other.id, DONE).unwrap();
    assert_eq!(
        epics(&db, false).unwrap().len(),
        1,
        "closed epics are hidden"
    );
    assert_eq!(epics(&db, true).unwrap().len(), 2);
}

#[test]
fn only_epics_can_be_linked_and_deleting_one_keeps_its_tasks() {
    let mut db = db();
    let e = create_epic(&mut db, "E").unwrap();
    let plain = task(&mut db, "plain");
    assert!(
        create(
            &mut db,
            &NewTask {
                epic: Some(plain.id),
                ..NewTask::new("x")
            }
        )
        .is_err()
    );
    assert!(
        create(
            &mut db,
            &NewTask {
                epic: Some(999),
                ..NewTask::new("x")
            }
        )
        .is_err()
    );

    let t = create(
        &mut db,
        &NewTask {
            epic: Some(e.id),
            ..NewTask::new("t")
        },
    )
    .unwrap();
    let moved = edit(
        &mut db,
        t.id,
        &Edit {
            epic: Some(None),
            ..Edit::default()
        },
    )
    .unwrap();
    assert_eq!(moved.epic, None);
    edit(
        &mut db,
        t.id,
        &Edit {
            epic: Some(Some(e.id)),
            ..Edit::default()
        },
    )
    .unwrap();
    delete_epic(&mut db, e.id).unwrap();
    let kept = all(&db, &Filter::default());
    assert!(kept.iter().any(|x| x.id == t.id && x.epic.is_none()));
    assert!(
        delete_epic(&mut db, plain.id).is_err(),
        "a task is not an epic"
    );
}

#[test]
fn set_tags_adds_and_removes_the_difference() {
    let mut db = db();
    let t = create(
        &mut db,
        &NewTask {
            tags: vec!["a".into(), "b".into()],
            ..NewTask::new("t")
        },
    )
    .unwrap();
    set_tags(&mut db, t.id, &["B".into(), "c".into()]).unwrap();
    assert_eq!(db.get(t.id).unwrap().tags, ["b", "c"]);
    set_tags(&mut db, t.id, &[]).unwrap();
    assert!(db.get(t.id).unwrap().tags.is_empty());
}

#[test]
fn forest_promotes_tasks_whose_parent_is_filtered_out() {
    let mut db = db();
    let a = create(
        &mut db,
        &NewTask {
            tags: vec!["x".into()],
            ..NewTask::new("a")
        },
    )
    .unwrap();
    create(
        &mut db,
        &NewTask {
            parent: Some(a.id),
            tags: vec!["y".into()],
            ..NewTask::new("b")
        },
    )
    .unwrap();
    let f = Filter {
        tags: TagFilter {
            all: vec!["y".into()],
            ..Default::default()
        },
        ..Filter::default()
    };
    let roots = forest(&db, &f, d(TODAY)).unwrap();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].value.title, "b");
}

#[test]
fn edit_sets_and_clears_dates_notes_and_priority() {
    let mut db = db();
    let t = create(
        &mut db,
        &NewTask {
            due: Some(d("2026-11-20")),
            ..NewTask::new("t")
        },
    )
    .unwrap();
    let e = Edit {
        notes: Some("hello".into()),
        due: Some(None),
        start: Some(Some(d("2026-11-15"))),
        priority: Some(Priority::Highest),
        ..Edit::default()
    };
    let t = edit(&mut db, t.id, &e).unwrap();
    assert_eq!(
        (t.notes.as_str(), t.due, t.start, t.priority),
        ("hello", None, Some(d("2026-11-15")), Priority::Highest)
    );
    let t = edit(
        &mut db,
        t.id,
        &Edit {
            notes: Some(String::new()),
            ..Edit::default()
        },
    )
    .unwrap();
    assert_eq!(t.notes, "");
}

#[test]
fn task_status_commands_refuse_epics() {
    let mut db = db();
    let e = create_epic(&mut db, "E").unwrap();
    assert!(set_status(&mut db, e.id, DONE).is_err());
    assert_eq!(
        set_epic_status(&mut db, e.id, DONE).unwrap().status,
        Some(DONE)
    );
}

#[test]
fn the_board_groups_tasks_by_status_into_the_views_columns() {
    use todo_schema::board::{BoardView, columns};
    let mut db = db();
    let a = task(&mut db, "todo");
    let b = task(&mut db, "doing");
    let c = task(&mut db, "done");
    let e = task(&mut db, "wont");
    set_status(&mut db, b.id, DOING).unwrap();
    set_status(&mut db, c.id, DONE).unwrap();
    set_status(&mut db, e.id, WONT).unwrap();
    let f = Filter {
        show_closed: true,
        ..Filter::default()
    };
    let tasks = all(&db, &f);
    let names = |v: &[(&Status, Vec<&Task>)]| {
        v.iter()
            .map(|(s, t)| (s.id, t.iter().map(|t| t.title.clone()).collect::<Vec<_>>()))
            .collect::<Vec<_>>()
    };
    let st = statuses::load(&db).unwrap();
    // Without "show closed" only the open statuses are columns, and finished work has none.
    assert_eq!(
        names(&columns(&tasks, BoardView::Open, &st)),
        [
            (OPEN, vec!["todo".to_string()]),
            (DOING, vec!["doing".into()]),
        ]
    );
    // With it, every status is a column, finished ones after the open ones.
    assert_eq!(
        names(&columns(&tasks, BoardView::All, &st)),
        [
            (OPEN, vec!["todo".to_string()]),
            (DOING, vec!["doing".into()]),
            (DONE, vec!["done".into()]),
            (WONT, vec!["wont".into()]),
        ]
    );
    let _ = a;
}

#[test]
fn several_picked_tags_are_alternatives_not_requirements() {
    let mut db = db();
    let add = |db: &mut Db, title: &str, tags: &[&str]| {
        create(
            db,
            &NewTask {
                tags: tags.iter().map(|t| t.to_string()).collect(),
                ..NewTask::new(title)
            },
        )
        .unwrap();
    };
    add(&mut db, "both", &["home", "work"]);
    add(&mut db, "only home", &["home"]);
    add(&mut db, "only work", &["work"]);
    add(&mut db, "neither", &["travel"]);
    let picked = |tags: &[&str]| {
        let f = Filter {
            tags: TagFilter {
                any: tags.iter().map(|t| t.to_string()).collect(),
                ..Default::default()
            },
            ..Filter::default()
        };
        titles(&all(&db, &f)).join(",")
    };
    assert_eq!(picked(&["home"]), "both,only home");
    assert_eq!(
        picked(&["home", "work"]),
        "both,only home,only work",
        "any one of the picked tags"
    );
    assert_eq!(
        picked(&[]),
        "both,only home,only work,neither",
        "nothing picked narrows nothing"
    );
}

// ---- the person's own statuses ---------------------------------------------------------------

fn names(db: &Db) -> Vec<String> {
    statuses::load(db)
        .unwrap()
        .all()
        .iter()
        .map(|s| s.name.clone())
        .collect()
}

#[test]
fn a_new_database_starts_with_the_four_familiar_statuses_once() {
    let mut db = db();
    assert_eq!(names(&db), ["To do", "In progress", "Done", "Won't do"]);
    let all = statuses::load(&db).unwrap();
    assert_eq!(
        all.all().iter().map(|s| s.closed).collect::<Vec<_>>(),
        [false, false, true, true]
    );
    assert!(all.all().iter().all(|s| s.color.is_some()));
    ensure_defaults(&mut db).unwrap();
    assert_eq!(names(&db).len(), 4, "running it again adds nothing");
}

#[test]
fn data_from_before_custom_statuses_is_moved_over_by_name() {
    use organiser_engine::{Field, FieldType, Kind, Schema};
    // The schema as it was: status a fixed list of names.
    static OLD: Schema = Schema {
        kinds: &[
            Kind {
                name: "task",
                fields: &[Field::new(
                    "status",
                    FieldType::Enum(&["open", "in_progress", "done", "dismissed"]),
                )],
            },
            Kind {
                name: "epic",
                fields: &[Field::new(
                    "status",
                    FieldType::Enum(&["open", "in_progress", "done", "dismissed"]),
                )],
            },
        ],
    };
    let path = std::env::temp_dir().join(format!("todo-legacy-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let (a, b, e) = {
        let mut old = Db::open(&path, &OLD).unwrap();
        let mk = |db: &mut Db, kind: &str, title: &str, status: &str| {
            db.create(organiser_engine::NewItem::new(kind, title).field("status", status))
                .unwrap()
                .id
        };
        (
            mk(&mut old, "task", "a", "done"),
            mk(&mut old, "task", "b", "in_progress"),
            mk(&mut old, "epic", "e", "dismissed"),
        )
    };
    // Opened by the new version.
    let mut db = Db::open(&path, &SCHEMA).unwrap();
    ensure_defaults(&mut db).unwrap();
    let all = statuses::load(&db).unwrap();
    let id = |n: &str| Some(all.by_name(n).unwrap().id);
    assert_eq!(Task::from(&db.get(a).unwrap()).status, id("Done"));
    assert_eq!(Task::from(&db.get(b).unwrap()).status, id("In progress"));
    assert_eq!(
        todo_schema::task::Epic::from(&db.get(e).unwrap()).status,
        id("Won't do")
    );
    drop(db);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn new_tasks_start_in_the_first_open_status_whatever_it_is_called() {
    let mut db = db();
    statuses::edit(
        &mut db,
        OPEN,
        &StatusEdit {
            name: Some("Backlog".into()),
            ..StatusEdit::default()
        },
    )
    .unwrap();
    assert_eq!(task(&mut db, "a").status, Some(OPEN));
    // Reordering changes which is first.
    statuses::move_by(&mut db, DOING, -1).unwrap();
    assert_eq!(names(&db)[0], "In progress");
    assert_eq!(task(&mut db, "b").status, Some(DOING));
}

#[test]
fn a_custom_closed_status_finishes_a_task_and_its_subtasks() {
    let mut db = db();
    let shipped = statuses::create(&mut db, "Shipped", Some("#ff8800"), true).unwrap();
    let a = task(&mut db, "a");
    let b = create(
        &mut db,
        &NewTask {
            parent: Some(a.id),
            ..NewTask::new("b")
        },
    )
    .unwrap();
    assert_eq!(set_status(&mut db, a.id, shipped.id).unwrap().len(), 2);
    assert_eq!(Task::from(&db.get(b.id).unwrap()).status, Some(shipped.id));
    assert!(
        all(&db, &Filter::default()).is_empty(),
        "closed tasks are hidden"
    );
    // Reopening into a custom open status reopens the parent too.
    let review = statuses::create(&mut db, "Review", None, false).unwrap();
    set_status(&mut db, b.id, review.id).unwrap();
    assert_eq!(Task::from(&db.get(a.id).unwrap()).status, Some(review.id));
}

#[test]
fn a_task_with_no_status_shows_as_open() {
    use organiser_engine::NewItem;
    let mut db = db();
    // Made through the generic `add`, which cannot know status ids.
    db.create(NewItem::new("task", "plain")).unwrap();
    assert_eq!(titles(&all(&db, &Filter::default())), ["plain"]);
}

#[test]
fn unknown_statuses_and_colours_are_refused() {
    let mut db = db();
    let a = task(&mut db, "a");
    assert!(set_status(&mut db, a.id, 999).is_err());
    assert!(statuses::create(&mut db, "Bad", Some("red"), false).is_err());
    assert!(
        statuses::create(&mut db, "to DO", None, false).is_err(),
        "names are unique, ignoring case"
    );
    assert!(statuses::create(&mut db, " ", None, false).is_err());
}

#[test]
fn there_is_always_an_open_and_a_closed_status() {
    let mut db = db();
    for id in [OPEN, DOING] {
        statuses::delete(&mut db, id).ok();
    }
    assert!(statuses::load(&db).unwrap().first_open().is_some());
    let last_open = statuses::load(&db).unwrap().first_open().unwrap().id;
    assert!(statuses::delete(&mut db, last_open).is_err());
    assert!(
        statuses::edit(
            &mut db,
            last_open,
            &StatusEdit {
                closed: Some(true),
                ..StatusEdit::default()
            }
        )
        .is_err()
    );
    let err = statuses::delete(&mut db, last_open)
        .unwrap_err()
        .to_string();
    assert!(err.contains("at least one open and one closed"), "{err}");
}

#[test]
fn deleting_a_status_moves_its_tasks_to_a_status_of_the_same_kind() {
    let mut db = db();
    let a = task(&mut db, "a");
    let b = task(&mut db, "b");
    set_status(&mut db, a.id, DOING).unwrap();
    set_status(&mut db, b.id, DONE).unwrap();
    statuses::delete(&mut db, DOING).unwrap();
    statuses::delete(&mut db, DONE).unwrap();
    let all = statuses::load(&db).unwrap();
    assert_eq!(
        Task::from(&db.get(a.id).unwrap()).status,
        Some(OPEN),
        "to the first open one"
    );
    assert_eq!(
        Task::from(&db.get(b.id).unwrap()).status,
        Some(WONT),
        "to the next closed one"
    );
    assert!(all.get(DOING).is_none());
}

#[test]
fn statuses_can_be_renamed_recoloured_and_reordered() {
    let mut db = db();
    let s = statuses::edit(
        &mut db,
        DONE,
        &StatusEdit {
            name: Some("Shipped".into()),
            color: Some(Some("#AABBCC".into())),
            ..StatusEdit::default()
        },
    )
    .unwrap();
    assert_eq!(
        (s.name.as_str(), s.color.as_deref()),
        ("Shipped", Some("#aabbcc"))
    );
    let s = statuses::edit(
        &mut db,
        DONE,
        &StatusEdit {
            color: Some(None),
            ..StatusEdit::default()
        },
    )
    .unwrap();
    assert_eq!(s.color, None);
    statuses::move_by(&mut db, WONT, -1).unwrap();
    assert_eq!(names(&db), ["To do", "In progress", "Won't do", "Shipped"]);
    statuses::move_by(&mut db, OPEN, 1).unwrap();
    assert_eq!(names(&db), ["In progress", "To do", "Won't do", "Shipped"]);
    statuses::move_by(&mut db, DOING, -1).unwrap();
    assert_eq!(names(&db)[0], "In progress", "no change at the edge");
}

#[test]
fn a_task_can_be_created_with_everything_the_edit_page_has() {
    let mut db = db();
    let epic = create_epic(&mut db, "E").unwrap();
    let t = create(
        &mut db,
        &NewTask {
            title: "Full".into(),
            notes: "- [ ] one\n".into(),
            epic: Some(epic.id),
            start: Some(d("2026-11-01")),
            due: Some(d("2026-11-20")),
            priority: Some(Priority::High),
            status: Some(DOING),
            tags: vec!["work".into()],
            ..NewTask::default()
        },
    )
    .unwrap();
    assert_eq!(t.notes, "- [ ] one\n");
    assert_eq!(
        (t.epic, t.status, t.priority),
        (Some(epic.id), Some(DOING), Priority::High)
    );
    assert_eq!(
        (t.start, t.due),
        (Some(d("2026-11-01")), Some(d("2026-11-20")))
    );
    assert_eq!(t.tags, ["work"]);
}

#[test]
fn schema_filters_pick_tasks_by_status_and_priority_and_asking_for_a_status_shows_closed_ones() {
    use organiser_engine::{Filters, filterable};
    use serde_json::json;
    let kind = SCHEMA.kinds.iter().find(|k| k.name == "task").unwrap();
    let offered: Vec<_> = filterable(kind).iter().map(|f| f.name).collect();
    assert_eq!(
        offered,
        ["status", "priority", "start", "due"],
        "epics have their own filter"
    );
    let mut db = db();
    let a = create(
        &mut db,
        &NewTask {
            priority: Some(Priority::High),
            ..NewTask::new("a")
        },
    )
    .unwrap();
    let b = task(&mut db, "b");
    let c = task(&mut db, "c");
    set_status(&mut db, c.id, DONE).unwrap();
    let with = |build: &dyn Fn(&mut Filters)| {
        let mut fields = Filters::default();
        build(&mut fields);
        titles_of(&all(
            &db,
            &Filter {
                fields,
                ..Filter::default()
            },
        ))
    };
    assert_eq!(with(&|f| f.toggle("priority", json!("high"))), ["a"]);
    assert_eq!(with(&|f| f.toggle("status", json!(OPEN))), ["a", "b"]);
    assert_eq!(
        with(&|f| f.toggle("status", json!(DONE))),
        ["c"],
        "picking a closed status shows its tasks without turning on 'show closed'"
    );
    assert_eq!(
        with(&|f| {
            f.toggle("status", json!(OPEN));
            f.toggle("status", json!(DONE));
        }),
        ["a", "b", "c"],
        "either of two"
    );
    let _ = (a, b);
}

fn titles_of(v: &[Task]) -> Vec<String> {
    v.iter().map(|t| t.title.clone()).collect()
}
