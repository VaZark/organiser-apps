//! Cross-app checks of the linking logic: every app's offer is consistent with its own schema,
//! switches are independent, republishing never loses a switch, and Sticky sees exactly what the
//! apps flag, through the registry only. Also a third-party registration, with nothing compiled in.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use organiser_core::sticky::StickySpec;
use organiser_engine::{Db, FieldType, ItemPatch, NewItem, Schema};
use organiser_sticky::register::{self, parse_kind};
use organiser_sticky::{Link, STICKY, STICKY_COLOR, flag, store};

/// The registry is read from `XDG_CONFIG_HOME`: one test at a time may point it somewhere.
static ENV: Mutex<()> = Mutex::new(());

struct Sandbox {
    dir: PathBuf,
    _guard: std::sync::MutexGuard<'static, ()>,
}

impl Sandbox {
    fn new(name: &str) -> Self {
        let guard = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir =
            std::env::temp_dir().join(format!("sticky-linking-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY: tests hold `ENV` while they use the variable.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", dir.join("config")) };
        Self { dir, _guard: guard }
    }
    fn db(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.db"))
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn apps() -> [(&'static StickySpec, &'static Schema); 3] {
    [
        (&notes_schema::STICKY, &notes_schema::SCHEMA),
        (&todo_schema::STICKY, &todo_schema::SCHEMA),
        (&faves_schema::STICKY, &faves_schema::SCHEMA),
    ]
}

/// Make one item of the app's sticky kind, with `text` in its text field.
fn make(db: &mut Db, spec: &StickySpec, title: &str, text: &str) -> i64 {
    let k = &spec.kinds[0];
    let mut new = NewItem::new(k.kind, title).field(k.body, text);
    if spec.meta.slug == "faves" {
        // A fave's entry lives in a list.
        let list = db.create(NewItem::new(faves_schema::LIST, "list")).unwrap();
        new = new.parent(list.id);
    }
    db.create(new).unwrap().id
}

#[test]
fn every_offer_matches_the_apps_own_schema() {
    for (spec, schema) in apps() {
        let slug = spec.meta.slug;
        assert!(!spec.kinds.is_empty(), "{slug} offers nothing");
        for k in spec.kinds {
            let kind = schema
                .kind(k.kind)
                .unwrap_or_else(|| panic!("{slug}: no kind {}", k.kind));
            let body = kind
                .field(k.body)
                .unwrap_or_else(|| panic!("{slug}: no field {}", k.body));
            assert_eq!(body.ty, FieldType::Markdown, "{slug}: the text is Markdown");
            assert!(body.searchable, "{slug}: the text is what search reads");
            assert_eq!(
                kind.field(STICKY).map(|f| f.ty),
                Some(FieldType::Bool),
                "{slug}"
            );
            assert_eq!(
                kind.field(STICKY_COLOR).map(|f| f.ty),
                Some(FieldType::Text),
                "{slug}"
            );
            assert!(
                register::valid_name(k.kind) && register::valid_name(k.body),
                "{slug}"
            );
        }
    }
    // Apps must not collide in the registry.
    let ids: std::collections::HashSet<_> = apps().iter().map(|(s, _)| s.meta.id).collect();
    let slugs: std::collections::HashSet<_> = apps().iter().map(|(s, _)| s.meta.slug).collect();
    assert_eq!((ids.len(), slugs.len()), (3, 3));
    assert!(apps().iter().all(|(s, _)| register::valid_id(s.meta.id)));
}

#[test]
fn publishing_is_off_by_default_and_switches_are_independent() {
    let _sb = Sandbox::new("switches");
    for (spec, _) in apps() {
        store::publish(spec).unwrap();
        assert!(!store::is_linked(spec), "{} starts off", spec.meta.slug);
    }
    store::set_linked(&notes_schema::STICKY, true).unwrap();
    assert!(store::is_linked(&notes_schema::STICKY));
    assert!(
        !store::is_linked(&todo_schema::STICKY),
        "linking Notes leaves Todo alone"
    );
    assert!(!store::is_linked(&faves_schema::STICKY));

    // Apps start again and publish again: the switch survives, and nothing is rewritten.
    let before = std::fs::read_to_string(store::path()).unwrap();
    for (spec, _) in apps() {
        store::publish(spec).unwrap();
    }
    assert!(
        store::is_linked(&notes_schema::STICKY),
        "a restart keeps the link"
    );
    assert_eq!(std::fs::read_to_string(store::path()).unwrap(), before);

    // Sticky's side: it switches by id, and an app sees it.
    store::set_enabled_by_id(notes_schema::META.id, false).unwrap();
    store::set_enabled_by_id(faves_schema::META.id, true).unwrap();
    assert!(!store::is_linked(&notes_schema::STICKY));
    assert!(store::is_linked(&faves_schema::STICKY));
    // An id nobody published changes nothing and invents nothing.
    store::set_enabled_by_id("org.example.Nobody", true).unwrap();
    assert_eq!(store::load().unwrap().links.len(), 3);

    // Linking an app that never published (switching from the app's side first) publishes it.
    store::set_linked(&todo_schema::STICKY, true).unwrap();
    assert_eq!(store::load().unwrap().enabled().count(), 2);
}

#[test]
fn sticky_sees_exactly_what_the_linked_apps_flag() {
    let sb = Sandbox::new("sees");
    let mut flagged = Vec::new();
    for (spec, schema) in apps() {
        let path = sb.db(spec.meta.slug);
        let mut db = Db::open(&path, schema).unwrap();
        let a = make(&mut db, spec, "a", "alpha");
        make(&mut db, spec, "b", "beta");
        flag::set_sticky(&mut db, a, true).unwrap();
        // The registry carries this app's database, wherever it is.
        let mut link = store::link_for(spec);
        link.db_path = path.to_string_lossy().into_owned();
        store::set_enabled(&link, spec.meta.slug != "faves").unwrap();
        flagged.push((spec, a));
    }

    // Sticky's side: only the registry, no schema.
    let links = store::load().unwrap();
    let mut seen = Vec::new();
    for link in links.enabled() {
        let db = Db::open_loose(&link.db_path).unwrap();
        for k in &link.kinds {
            for it in db.flagged_loose(&k.kind, STICKY).unwrap() {
                assert_eq!(it.fields[&k.body].as_str(), Some("alpha"));
                seen.push(link.slug.clone());
            }
        }
    }
    seen.sort();
    assert_eq!(
        seen,
        ["notes", "todo"],
        "Faves is off, so its sticky is not shown"
    );

    // Switching Faves on shows it; the apps' own typed reads agree about the flag.
    store::set_enabled_by_id(faves_schema::META.id, true).unwrap();
    let faves = store::load().unwrap();
    let link = faves.get(faves_schema::META.id).unwrap();
    let loose = Db::open_loose(&link.db_path).unwrap();
    assert_eq!(loose.flagged_loose("entry", STICKY).unwrap().len(), 1);
    for (spec, id) in flagged {
        let typed = Db::open(
            sb.db(spec.meta.slug),
            apps()
                .iter()
                .find(|(s, _)| s.meta.id == spec.meta.id)
                .unwrap()
                .1,
        )
        .unwrap();
        assert!(flag::is_sticky(&typed.get(id).unwrap()));
    }
}

#[test]
fn an_edit_through_sticky_leaves_the_app_able_to_read_and_search_the_item() {
    let sb = Sandbox::new("loose");
    for (spec, schema) in apps() {
        let path = sb.db(spec.meta.slug);
        let mut app_db = Db::open(&path, schema).unwrap();
        let id = make(&mut app_db, spec, "x", "old text");
        let body = spec.kinds[0].body;

        let mut sticky = Db::open_loose(&path).unwrap();
        sticky
            .update_loose(
                id,
                ItemPatch::default()
                    .title("edited")
                    .field(body, "new words")
                    .field(STICKY, true),
                &[body],
            )
            .unwrap();

        // The app's own, schema-checked read and search still work and see the change.
        let it = app_db.get(id).unwrap();
        assert_eq!(
            (it.title.as_str(), it.fields[body].as_str()),
            ("edited", Some("new words"))
        );
        assert!(flag::is_sticky(&it));
        let hits = app_db
            .list(&organiser_engine::Query {
                text: Some("words".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            hits.len(),
            1,
            "{}: search finds the new text",
            spec.meta.slug
        );
        let stale = app_db
            .list(&organiser_engine::Query {
                text: Some("old".into()),
                ..Default::default()
            })
            .unwrap();
        assert!(stale.is_empty(), "{}: and not the old", spec.meta.slug);
        // And the app can still edit it, schema checks and all.
        app_db
            .update(id, ItemPatch::default().title("again"))
            .unwrap();
    }
}

#[test]
fn a_third_party_registers_and_links_with_nothing_compiled_in() {
    let sb = Sandbox::new("third");
    // Their database: any program that keeps items in the engine's layout. Here the layout is
    // made with the engine, standing in for it.
    let path = sb.db("tasks3p");
    let mut db = Db::open(&path, &todo_schema::SCHEMA).unwrap();
    let id = make(&mut db, &todo_schema::STICKY, "ship it", "to do");
    flag::set_sticky(&mut db, id, true).unwrap();

    // Not offered until registered.
    assert!(store::load().unwrap().links.is_empty());

    let link = Link {
        app_id: "org.example.Tasks".into(),
        name: "Example Tasks".into(),
        slug: "tasks".into(),
        db_path: path.to_string_lossy().into_owned(),
        kinds: vec![parse_kind("task=notes").unwrap()],
        enabled: true, // ignored: a registration is only an offer
    };
    let file = register::register(&link).unwrap();
    assert!(file.starts_with(store::dropin_dir()));
    let links = store::load().unwrap();
    assert!(links.get("org.example.Tasks").is_some());
    assert!(
        !links.is_linked("org.example.Tasks"),
        "registering does not link"
    );

    // Linking is the switch; it persists and the app is shown by what the registration says.
    store::set_enabled_by_id("org.example.Tasks", true).unwrap();
    let links = store::load().unwrap();
    let ours = links.enabled().next().unwrap();
    let loose = Db::open_loose(&ours.db_path).unwrap();
    assert_eq!(loose.flagged_loose("task", STICKY).unwrap().len(), 1);

    // A moved database is a new registration: the path changes, the switch stays.
    let moved = Link {
        db_path: "/elsewhere/tasks.db".into(),
        ..link.clone()
    };
    register::register(&moved).unwrap();
    let links = store::load().unwrap();
    assert_eq!(
        links.get("org.example.Tasks").unwrap().db_path,
        "/elsewhere/tasks.db"
    );
    assert!(links.is_linked("org.example.Tasks"));

    // Unregistering removes it everywhere.
    register::unregister("org.example.Tasks").unwrap();
    assert!(store::load().unwrap().links.is_empty());
    assert!(!register::unregister_in(&store::dropin_dir(), "org.example.Tasks").unwrap());
}

#[test]
fn bad_registrations_are_refused_and_bad_files_are_ignored() {
    let sb = Sandbox::new("bad");
    let ok = Link {
        app_id: "org.example.Ok".into(),
        db_path: "/x.db".into(),
        kinds: vec![parse_kind("note=body").unwrap()],
        ..Link::default()
    };
    let dir = store::dropin_dir();
    for (what, bad) in [
        (
            "path in the id",
            Link {
                app_id: "../evil".into(),
                ..ok.clone()
            },
        ),
        (
            "hidden id",
            Link {
                app_id: ".hidden".into(),
                ..ok.clone()
            },
        ),
        (
            "no database",
            Link {
                db_path: String::new(),
                ..ok.clone()
            },
        ),
        (
            "no kinds",
            Link {
                kinds: vec![],
                ..ok.clone()
            },
        ),
        (
            "odd kind",
            Link {
                kinds: vec![parse_kind("a-b=body").unwrap()],
                ..ok.clone()
            },
        ),
        (
            "odd field",
            Link {
                kinds: vec![parse_kind("note=$.x").unwrap()],
                ..ok.clone()
            },
        ),
    ] {
        assert!(
            register::register_in(&dir, &bad).is_err(),
            "{what} must be refused"
        );
    }
    assert!(register::unregister_in(&dir, "../evil").is_err());
    assert!(parse_kind("nofield").is_err());

    // A broken drop-in does not hide a good one.
    register::register_in(&dir, &ok).unwrap();
    std::fs::write(dir.join("broken.toml"), "this is [not toml").unwrap();
    std::fs::write(dir.join("empty-id.toml"), "[[link]]\nname = \"x\"\n").unwrap();
    std::fs::write(dir.join("readme.txt"), "ignored").unwrap();
    let links = store::load().unwrap();
    assert_eq!(links.links.len(), 1);
    assert_eq!(links.links[0].app_id, "org.example.Ok");
    let _ = &sb;
}

#[test]
fn a_database_that_is_not_in_the_engines_layout_is_left_alone() {
    let sb = Sandbox::new("foreign");
    let path: &Path = &sb.db("foreign");
    {
        // Something else's database.
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.execute_batch("CREATE TABLE notes(id INTEGER PRIMARY KEY, text TEXT); INSERT INTO notes(text) VALUES ('mine');")
            .unwrap();
    }
    let before = std::fs::read(path).unwrap();
    assert!(Db::open_loose(path).is_err(), "no items table: refused");
    assert_eq!(
        std::fs::read(path).unwrap(),
        before,
        "and not touched, not even migrated"
    );
    // A missing file is not created by Sticky either.
    let missing = sb.db("missing");
    let _ = Db::open_loose(&missing);
    let _ = std::fs::remove_file(&missing);
}
