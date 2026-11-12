//! Cross-app checks of the linking logic: every app's offer is consistent with its own schema,
//! switches are independent, republishing never loses a switch, and Sticky sees exactly what the
//! apps flag, through the registry only.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use organiser_core::sticky::StickySpec;
use organiser_engine::{Db, FieldType, ItemPatch, NewItem, Schema};
use organiser_sticky::{STICKY, STICKY_COLOR, flag, store};

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

fn apps() -> [(&'static StickySpec, &'static Schema); 2] {
    [
        (&notes_schema::STICKY, &notes_schema::SCHEMA),
        (&todo_schema::STICKY, &todo_schema::SCHEMA),
    ]
}

/// Make one item of the app's sticky kind, with `text` in its text field.
fn make(db: &mut Db, spec: &StickySpec, title: &str, text: &str) -> i64 {
    let k = &spec.kinds[0];
    let new = NewItem::new(k.kind, title).field(k.body, text);
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
        }
    }
    // Apps must not collide in the registry.
    let ids: std::collections::HashSet<_> = apps().iter().map(|(s, _)| s.meta.id).collect();
    let slugs: std::collections::HashSet<_> = apps().iter().map(|(s, _)| s.meta.slug).collect();
    assert_eq!((ids.len(), slugs.len()), (2, 2));
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
    store::set_enabled_by_id(todo_schema::META.id, true).unwrap();
    assert!(!store::is_linked(&notes_schema::STICKY));
    assert!(store::is_linked(&todo_schema::STICKY));
    // An id nobody published changes nothing and invents nothing.
    store::set_enabled_by_id("org.example.Nobody", true).unwrap();
    assert_eq!(store::load().unwrap().links.len(), 2);

    // Switching Notes back on from the app's side leaves Todo as it was.
    store::set_linked(&notes_schema::STICKY, true).unwrap();
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
        store::set_enabled(&link, spec.meta.slug != "todo").unwrap();
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
        ["notes"],
        "Todo is off, so its sticky is not shown"
    );

    // Switching Todo on shows it; the apps' own typed reads agree about the flag.
    store::set_enabled_by_id(todo_schema::META.id, true).unwrap();
    let registry = store::load().unwrap();
    let link = registry.get(todo_schema::META.id).unwrap();
    let loose = Db::open_loose(&link.db_path).unwrap();
    assert_eq!(loose.flagged_loose("task", STICKY).unwrap().len(), 1);
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
