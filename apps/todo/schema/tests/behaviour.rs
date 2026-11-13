use chrono::NaiveDate;
use organiser_engine::{Db, TagFilter};
use todo_schema::SCHEMA;
use todo_schema::lifecycle::{
    NewTask, create, create_epic, delete_epic, edit, set_status, set_tags,
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
fn current_is_what_is_in_progress_each_task_alone() {
    use todo_schema::lifecycle::set_status;
    let mut db = db();
    let parent = create(&mut db, &NewTask::new("parent")).unwrap();
    let working = create(
        &mut db,
        &NewTask {
            parent: Some(parent.id),
            ..NewTask::new("working subtask")
        },
    )
    .unwrap();
    create(
        &mut db,
        &NewTask {
            parent: Some(parent.id),
            ..NewTask::new("idle sibling")
        },
    )
    .unwrap();
    // Dates do not make a task current any more.
    create(&mut db, &NewTask { due: Some(d("2026-11-12")), ..NewTask::new("due, not started") }).unwrap();
    let other = create(&mut db, &NewTask::new("also working")).unwrap();
    let in_progress = todo_schema::statuses::load(&db).unwrap().by_name("In progress").unwrap().id;
    set_status(&mut db, working.id, in_progress).unwrap();
    set_status(&mut db, other.id, in_progress).unwrap();
    let current = all(
        &db,
        &Filter {
            scope: Scope::Current,
            ..Filter::default()
        },
    );
    let mut names = titles(&current);
    names.sort();
    assert_eq!(names, ["also working", "working subtask"]);
    let sub = current.iter().find(|t| t.id == working.id).unwrap();
    assert_eq!(sub.parent_id, Some(parent.id), "it still knows its parent");
}

#[test]
fn archived_tasks_are_their_own_view_and_leave_the_others() {
    use todo_schema::lifecycle::set_archived;
    let mut db = db();
    let parent = create(&mut db, &NewTask::new("parent")).unwrap();
    let sub = create(&mut db, &NewTask { parent: Some(parent.id), ..NewTask::new("sub") }).unwrap();
    create(&mut db, &NewTask::new("kept")).unwrap();
    set_archived(&mut db, parent.id, true).unwrap();
    let archived = |db: &Db| {
        all(db, &Filter { scope: Scope::Archived, ..Filter::default() })
    };
    // The parent and everything below it.
    assert_eq!(titles(&archived(&db)), ["parent", "sub"]);
    assert_eq!(titles(&all(&db, &Filter::default())), ["kept"]);
    // Bringing a subtask back brings the tasks above it too.
    set_archived(&mut db, sub.id, false).unwrap();
    assert!(archived(&db).is_empty());
    assert_eq!(all(&db, &Filter::default()).len(), 3);
    assert!(set_archived(&mut db, 9999, true).is_err());
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
    let list = epics(&db).unwrap();
    let holiday = list.iter().find(|s| s.epic.id == e.id).unwrap();
    assert_eq!(
        (holiday.open, holiday.total, holiday.progress()),
        (1, 2, 0.5)
    );

    // An epic has no status, so none is ever hidden for being finished.
    assert_eq!(epics(&db).unwrap().len(), 2);
    let _ = other;
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
    // An epic no longer has a status, but one stored by an older version is still migrated.
    assert_eq!(
        db.get(e).unwrap().fields.get("status").and_then(|v| v.as_i64()),
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
        ["pinned", "sticky", "status", "priority", "start", "due"],
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

#[test]
fn sections_belong_to_their_epic() {
    use todo_schema::lifecycle::{create_section, delete_section, edit_section};
    use todo_schema::views::sections;
    let mut db = db();
    let a = create_epic(&mut db, "A").unwrap();
    let b = create_epic(&mut db, "B").unwrap();
    let design = create_section(&mut db, a.id, "Design").unwrap();
    let other = create_section(&mut db, b.id, "Other").unwrap();

    let t = create(
        &mut db,
        &NewTask {
            epic: Some(a.id),
            section: Some(design.id),
            ..NewTask::new("t")
        },
    )
    .unwrap();
    assert_eq!(t.section, Some(design.id));

    // A section of another epic, or of none, is refused.
    let wrong = Edit {
        section: Some(Some(other.id)),
        ..Edit::default()
    };
    assert!(edit(&mut db, t.id, &wrong).is_err());

    // Progress counts the section's own tasks.
    let s = &sections(&db, a.id).unwrap()[0];
    assert_eq!((s.open, s.total), (1, 1));
    set_status(&mut db, t.id, DONE).unwrap();
    assert_eq!(sections(&db, a.id).unwrap()[0].progress(), 1.0);

    // Colour and description are the section's own.
    let e = Edit {
        notes: Some("why".into()),
        color: Some(Some("#112233".into())),
        ..Edit::default()
    };
    let s = edit_section(&mut db, design.id, &e).unwrap();
    assert_eq!((s.notes.as_str(), s.color.as_deref()), ("why", Some("#112233")));

    // Moving the task to another epic leaves its old section behind.
    let moved = Edit {
        epic: Some(Some(b.id)),
        ..Edit::default()
    };
    assert_eq!(edit(&mut db, t.id, &moved).unwrap().section, None);

    // Deleting a section keeps its tasks in the epic.
    let t2 = create(
        &mut db,
        &NewTask {
            epic: Some(b.id),
            section: Some(other.id),
            ..NewTask::new("t2")
        },
    )
    .unwrap();
    delete_section(&mut db, other.id).unwrap();
    let t2 = all(&db, &Filter::default()).into_iter().find(|t| t.id == t2.id).unwrap();
    assert_eq!((t2.epic, t2.section), (Some(b.id), None));
}

#[test]
fn reparenting_joins_the_new_parents_place() {
    use todo_schema::lifecycle::{create_section, reparent};
    let mut db = db();
    let e = create_epic(&mut db, "E").unwrap();
    let s = create_section(&mut db, e.id, "S").unwrap();
    let home = create(
        &mut db,
        &NewTask {
            epic: Some(e.id),
            section: Some(s.id),
            ..NewTask::new("home")
        },
    )
    .unwrap();
    let a = task(&mut db, "a");
    let child = create(
        &mut db,
        &NewTask {
            parent: Some(a.id),
            ..NewTask::new("child")
        },
    )
    .unwrap();

    // Nested, it takes the parent's epic and section, and so do its subtasks.
    let moved = reparent(&mut db, a.id, Some(home.id)).unwrap();
    assert_eq!((moved.parent_id, moved.epic, moved.section), (Some(home.id), Some(e.id), Some(s.id)));
    let all = all(&db, &Filter::default());
    let child = all.iter().find(|t| t.id == child.id).unwrap();
    assert_eq!((child.parent_id, child.epic, child.section), (Some(a.id), Some(e.id), Some(s.id)));

    // Never under itself or its own subtasks.
    assert!(reparent(&mut db, a.id, Some(a.id)).is_err());
    assert!(reparent(&mut db, a.id, Some(child.id)).is_err());

    // Out to the top level, it keeps its place.
    let out = reparent(&mut db, a.id, None).unwrap();
    assert_eq!((out.parent_id, out.epic), (None, Some(e.id)));
}

#[test]
fn subtasks_follow_their_task_to_another_epic_and_section() {
    use todo_schema::lifecycle::{create_section, set_pinned};
    let mut db = db();
    let a = create_epic(&mut db, "A").unwrap();
    let b = create_epic(&mut db, "B").unwrap();
    let sa = create_section(&mut db, a.id, "SA").unwrap();
    let sb = create_section(&mut db, b.id, "SB").unwrap();
    let mk = |db: &mut Db, title: &str, parent: Option<i64>| {
        create(
            db,
            &NewTask {
                epic: Some(a.id),
                section: Some(sa.id),
                parent,
                ..NewTask::new(title)
            },
        )
        .unwrap()
    };
    let parent = mk(&mut db, "parent", None);
    let child = mk(&mut db, "child", Some(parent.id));
    let grandchild = mk(&mut db, "grandchild", Some(child.id));
    let place = |db: &Db, id: i64| {
        let t = all(db, &Filter::default()).into_iter().find(|t| t.id == id).unwrap();
        (t.epic, t.section)
    };

    // A new epic: the section is cleared, for the task and for its subtasks.
    let to_b = Edit {
        epic: Some(Some(b.id)),
        ..Edit::default()
    };
    edit(&mut db, parent.id, &to_b).unwrap();
    for id in [parent.id, child.id, grandchild.id] {
        assert_eq!(place(&db, id), (Some(b.id), None));
    }
    // A section of that epic: they follow into it.
    let into = Edit {
        section: Some(Some(sb.id)),
        ..Edit::default()
    };
    edit(&mut db, parent.id, &into).unwrap();
    for id in [parent.id, child.id, grandchild.id] {
        assert_eq!(place(&db, id), (Some(b.id), Some(sb.id)));
    }
    // Other edits leave the subtasks alone.
    set_pinned(&mut db, parent.id, true).unwrap();
    assert_eq!(place(&db, child.id), (Some(b.id), Some(sb.id)));
}

#[test]
fn deleting_an_epic_keeps_or_deletes_its_tasks() {
    use todo_schema::lifecycle::delete_epic_and_tasks;
    let mut db = db();
    let keep = create_epic(&mut db, "keep").unwrap();
    let all_gone = create_epic(&mut db, "gone").unwrap();
    for (epic, title) in [(keep.id, "k1"), (all_gone.id, "g1")] {
        let t = create(
            &mut db,
            &NewTask {
                epic: Some(epic),
                ..NewTask::new(title)
            },
        )
        .unwrap();
        create(
            &mut db,
            &NewTask {
                epic: Some(epic),
                parent: Some(t.id),
                ..NewTask::new(&format!("{title} sub"))
            },
        )
        .unwrap();
    }
    // Keeping: the tasks stay, in no epic.
    delete_epic(&mut db, keep.id).unwrap();
    let stay = all(&db, &Filter::default());
    assert!(stay.iter().filter(|t| t.title.starts_with('k')).all(|t| t.epic.is_none()));
    assert_eq!(stay.len(), 4);
    // Deleting: the tasks and their subtasks go too.
    assert_eq!(delete_epic_and_tasks(&mut db, all_gone.id).unwrap(), 2);
    let left = all(&db, &Filter::default());
    assert_eq!(titles(&left), ["k1", "k1 sub"]);
}

#[test]
fn pinned_tasks_come_first() {
    use todo_schema::lifecycle::set_pinned;
    let mut db = db();
    let a = task(&mut db, "a");
    let b = task(&mut db, "b");
    let c = task(&mut db, "c");
    assert_eq!(titles(&all(&db, &Filter::default())), ["a", "b", "c"]);
    assert!(set_pinned(&mut db, c.id, true).unwrap().pinned);
    assert_eq!(titles(&all(&db, &Filter::default())), ["c", "a", "b"]);
    // Still in the person's order among the pinned.
    set_pinned(&mut db, b.id, true).unwrap();
    assert_eq!(titles(&all(&db, &Filter::default())), ["b", "c", "a"]);
    set_pinned(&mut db, c.id, false).unwrap();
    set_pinned(&mut db, b.id, false).unwrap();
    assert_eq!(titles(&all(&db, &Filter::default())), ["a", "b", "c"]);
    let _ = a;
}

#[test]
fn a_section_can_be_moved_to_another_place() {
    use todo_schema::lifecycle::{create_section, move_section};
    use todo_schema::views::sections;
    let mut db = db();
    let e = create_epic(&mut db, "E").unwrap();
    let a = create_section(&mut db, e.id, "A").unwrap();
    let b = create_section(&mut db, e.id, "B").unwrap();
    let c = create_section(&mut db, e.id, "C").unwrap();
    let order = |db: &Db| -> Vec<String> {
        sections(db, e.id).unwrap().into_iter().map(|s| s.section.title).collect()
    };
    assert_eq!(order(&db), ["A", "B", "C"]);
    move_section(&mut db, c.id, 0).unwrap();
    assert_eq!(order(&db), ["C", "A", "B"]);
    move_section(&mut db, c.id, 2).unwrap();
    assert_eq!(order(&db), ["A", "B", "C"]);
    move_section(&mut db, a.id, 1).unwrap();
    assert_eq!(order(&db), ["B", "A", "C"]);
    // Past the end is the end.
    move_section(&mut db, b.id, 99).unwrap();
    assert_eq!(order(&db), ["A", "C", "B"]);
    let _ = (a, b, c);
}

#[test]
fn an_epic_can_be_moved_before_another() {
    use todo_schema::lifecycle::move_epic;
    let mut db = db();
    let a = create_epic(&mut db, "A").unwrap();
    let b = create_epic(&mut db, "B").unwrap();
    let c = create_epic(&mut db, "C").unwrap();
    let order = |db: &Db| -> Vec<String> {
        epics(db).unwrap().into_iter().map(|s| s.epic.title).collect()
    };
    assert_eq!(order(&db), ["A", "B", "C"]);
    move_epic(&mut db, c.id, Some(a.id)).unwrap();
    assert_eq!(order(&db), ["C", "A", "B"]);
    move_epic(&mut db, c.id, None).unwrap();
    assert_eq!(order(&db), ["A", "B", "C"]);
    move_epic(&mut db, a.id, Some(c.id)).unwrap();
    assert_eq!(order(&db), ["B", "A", "C"]);
    // Before itself changes nothing.
    move_epic(&mut db, a.id, Some(a.id)).unwrap();
    assert_eq!(order(&db), ["B", "A", "C"]);
    assert!(move_epic(&mut db, a.id, Some(999)).is_err());
    let _ = b;
}

#[test]
fn tasks_in_no_epic_can_be_listed_and_counted() {
    use todo_schema::views::unassigned;
    let mut db = db();
    let e = create_epic(&mut db, "E").unwrap();
    create(&mut db, &NewTask { epic: Some(e.id), ..NewTask::new("in") }).unwrap();
    create(&mut db, &NewTask::new("out 1")).unwrap();
    let two = create(&mut db, &NewTask::new("out 2")).unwrap();
    let done = todo_schema::statuses::load(&db).unwrap().first_closed().unwrap().id;
    todo_schema::lifecycle::set_status(&mut db, two.id, done).unwrap();
    let none = all(&db, &Filter { no_epic: true, show_closed: true, ..Filter::default() });
    assert_eq!(titles(&none), ["out 1", "out 2"]);
    assert_eq!(unassigned(&db).unwrap(), (1, 2));
}
