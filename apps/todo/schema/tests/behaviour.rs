use chrono::NaiveDate;
use organiser_engine::{Db, TagFilter};
use todo_schema::SCHEMA;
use todo_schema::lifecycle::{
    NewTask, create, create_epic, delete_epic, edit, set_epic_status, set_status, set_tags,
};
use todo_schema::priority::Priority;
use todo_schema::status::Status;
use todo_schema::task::{Edit, Task};
use todo_schema::views::{Filter, Scope, epics, forest, subtasks, tasks};

fn d(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

fn titles(v: &[Task]) -> Vec<&str> {
    v.iter().map(|t| t.title.as_str()).collect()
}

fn db() -> Db {
    Db::open_in_memory(&SCHEMA).unwrap()
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
    assert_eq!((t.status, t.priority), (Status::Open, Priority::Medium));
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
    set_status(&mut db, a.id, Status::Done).unwrap();
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
    set_status(&mut db, a.id, Status::InProgress).unwrap();
    let open = all(&db, &Filter::default());
    assert_eq!(
        open.iter().map(|t| t.status).collect::<Vec<_>>(),
        [Status::InProgress, Status::Open]
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
    assert_eq!(set_status(&mut db, a.id, Status::Done).unwrap().len(), 3);
    let closed = Filter {
        show_closed: true,
        ..Filter::default()
    };
    assert!(all(&db, &Filter::default()).is_empty());
    assert_eq!(all(&db, &closed).len(), 3);

    set_status(&mut db, c.id, Status::Open).unwrap();
    assert_eq!(
        titles(&all(&db, &Filter::default())),
        ["a", "b", "c"],
        "ancestors reopen with it"
    );
    assert!(
        set_status(&mut db, c.id, Status::Open).unwrap().is_empty(),
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
    set_status(&mut db, b.id, Status::Done).unwrap();
    set_status(&mut db, a.id, Status::Dismissed).unwrap();
    let f = Filter {
        show_closed: true,
        ..Filter::default()
    };
    let got = all(&db, &f);
    let st = |n: &str| got.iter().find(|t| t.title == n).unwrap().status;
    assert_eq!((st("a"), st("b")), (Status::Dismissed, Status::Done));
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
    set_status(&mut db, a.id, Status::Done).unwrap();
    let list = epics(&db, false).unwrap();
    let holiday = list.iter().find(|s| s.epic.id == e.id).unwrap();
    assert_eq!(
        (holiday.open, holiday.total, holiday.progress()),
        (1, 2, 0.5)
    );

    set_epic_status(&mut db, other.id, Status::Done).unwrap();
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
    assert!(set_status(&mut db, e.id, Status::Done).is_err());
    assert_eq!(
        set_epic_status(&mut db, e.id, Status::Done).unwrap().status,
        Status::Done
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
    set_status(&mut db, b.id, Status::InProgress).unwrap();
    set_status(&mut db, c.id, Status::Done).unwrap();
    set_status(&mut db, e.id, Status::Dismissed).unwrap();
    let f = Filter {
        show_closed: true,
        ..Filter::default()
    };
    let tasks = all(&db, &f);
    let names = |v: &[(Status, Vec<&Task>)]| {
        v.iter()
            .map(|(s, t)| (*s, t.iter().map(|t| t.title.clone()).collect::<Vec<_>>()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        names(&columns(&tasks, BoardView::All)),
        [
            (Status::Open, vec!["todo".to_string()]),
            (Status::InProgress, vec!["doing".into()]),
            (Status::Done, vec!["done".into()])
        ]
    );
    assert_eq!(
        names(&columns(&tasks, BoardView::Closed)),
        [
            (Status::Done, vec!["done".to_string()]),
            (Status::Dismissed, vec!["wont".into()])
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
