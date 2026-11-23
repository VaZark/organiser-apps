//! The engine over real databases of the three apps: what it shows, that edits reach the app's
//! database, and that dropping a sticky clears the flag and nothing else.

use std::path::PathBuf;
use std::sync::Mutex;

use organiser_core::sticky::StickySpec;
use organiser_engine::{Db, NewItem, Query, Schema};
use organiser_sticky::{Link, flag, store};
use sticky_engine::{Engine, Key, Watcher};

/// The registry is read from `XDG_CONFIG_HOME`: one test at a time may point it somewhere.
static ENV: Mutex<()> = Mutex::new(());

struct Sandbox {
    dir: PathBuf,
    _guard: std::sync::MutexGuard<'static, ()>,
}

impl Sandbox {
    fn new(name: &str) -> Self {
        let guard = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("sticky-engine-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // SAFETY: tests hold `ENV` while they use the variable.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", dir.join("config")) };
        Self { dir, _guard: guard }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

struct Case {
    spec: &'static StickySpec,
    schema: &'static Schema,
}

fn cases() -> [Case; 2] {
    [
        Case {
            spec: &notes_schema::STICKY,
            schema: &notes_schema::SCHEMA,
        },
        Case {
            spec: &todo_schema::STICKY,
            schema: &todo_schema::SCHEMA,
        },
    ]
}

fn make(db: &mut Db, spec: &StickySpec, title: &str) -> i64 {
    let k = &spec.kinds[0];
    let new = NewItem::new(k.kind, title).field(k.body, "first");
    db.create(new).unwrap().id
}

fn link_to(sb: &Sandbox, case: &Case) -> PathBuf {
    let path = sb.dir.join(format!("{}.db", case.spec.meta.slug));
    let mut link = store::link_for(case.spec);
    link.db_path = path.to_string_lossy().into_owned();
    store::set_enabled(&link, true).unwrap();
    path
}

#[test]
fn refresh_shows_flagged_items_and_says_what_changed() {
    let sb = Sandbox::new("refresh");
    let case = &cases()[0];
    let path = link_to(&sb, case);
    let mut db = Db::open(&path, case.schema).unwrap();
    let a = make(&mut db, case.spec, "a");
    let b = make(&mut db, case.spec, "b");

    let mut e = Engine::new();
    assert!(e.refresh().unwrap().is_empty(), "nothing flagged yet");
    flag::set_sticky(&mut db, a, true).unwrap();
    let key = Key::new(case.spec.meta.id, a);
    assert_eq!(e.refresh().unwrap().added, std::slice::from_ref(&key));
    assert_eq!(e.items().len(), 1);
    assert_eq!(e.item(&key).unwrap().body, "first");
    assert!(
        e.refresh().unwrap().is_empty(),
        "looking again changes nothing"
    );

    // An edit made in the app shows up as an update.
    db.update(a, organiser_engine::ItemPatch::default().title("renamed"))
        .unwrap();
    assert_eq!(e.refresh().unwrap().updated, std::slice::from_ref(&key));
    assert_eq!(e.item(&key).unwrap().title, "renamed");

    // Unflagged, archived or trashed in the app: gone here.
    flag::set_sticky(&mut db, b, true).unwrap();
    assert_eq!(e.refresh().unwrap().added.len(), 1);
    db.update(b, organiser_engine::ItemPatch::default().archived(true))
        .unwrap();
    assert_eq!(
        e.refresh().unwrap().removed,
        [Key::new(case.spec.meta.id, b)]
    );
    flag::set_sticky(&mut db, a, false).unwrap();
    assert_eq!(e.refresh().unwrap().removed, [key]);
    assert!(e.items().is_empty());
}

#[test]
fn edits_reach_the_database_but_the_sticky_flag_is_the_apps_alone() {
    let sb = Sandbox::new("edits");
    for case in cases() {
        let slug = case.spec.meta.slug;
        let path = link_to(&sb, &case);
        let mut db = Db::open(&path, case.schema).unwrap();
        let id = make(&mut db, case.spec, "t");
        flag::set_sticky(&mut db, id, true).unwrap();
        let key = Key::new(case.spec.meta.id, id);
        let body = case.spec.kinds[0].body;

        let mut e = Engine::new();
        e.refresh().unwrap();
        assert_eq!(e.items().len(), 1, "{slug}");
        assert!(
            e.items()[0].show_on_desktop,
            "{slug}: on the desktop by default"
        );

        // Title and text are forwarded to the app's database, and to its search.
        e.set_title(&key, "renamed").unwrap();
        e.set_body(&key, "new words").unwrap();
        let it = db.get(id).unwrap();
        assert_eq!(it.title, "renamed", "{slug}");
        assert_eq!(it.fields[body].as_str(), Some("new words"), "{slug}");
        let hits = db
            .list(&Query {
                text: Some("words".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(hits.len(), 1, "{slug}: search follows the edit");

        // Colour and desktop visibility are Sticky's own: nothing of them reaches the app.
        let before = db.get(id).unwrap();
        e.set_color(&key, Some("#90caf9")).unwrap();
        e.set_show_on_desktop(&key, false).unwrap();
        assert_eq!(
            db.get(id).unwrap(),
            before,
            "{slug}: the app's item is untouched"
        );
        assert!(
            flag::is_sticky(&before),
            "{slug}: still a sticky in its app"
        );
        assert_eq!(flag::color(&before), None, "{slug}");
        let item = e.item(&key).unwrap();
        assert_eq!(item.color.as_deref(), Some("#90caf9"), "{slug}");
        assert!(!item.show_on_desktop, "{slug}");

        // A fresh engine (the next run) sees the same, and the sticky is still listed: only its
        // window is off.
        let mut again = Engine::new();
        again.refresh().unwrap();
        let item = again
            .item(&key)
            .expect("a hidden sticky stays in Sticky Notes");
        assert!(
            !item.show_on_desktop && item.color.as_deref() == Some("#90caf9"),
            "{slug}"
        );
        assert!(again.refresh().unwrap().is_empty(), "{slug}");

        // Back on the desktop, and back to the default paper.
        e.set_show_on_desktop(&key, true).unwrap();
        e.set_color(&key, None).unwrap();
        assert!(
            store::load_local().unwrap().items.is_empty(),
            "{slug}: defaults keep nothing"
        );
        assert_eq!(db.get(id).unwrap(), before, "{slug}");
        let _ = std::fs::remove_file(&path);
    }
}

#[test]
fn the_apps_own_colour_shows_until_sticky_picks_one() {
    let sb = Sandbox::new("appcolor");
    let case = &cases()[0];
    let path = link_to(&sb, case);
    let mut db = Db::open(&path, case.schema).unwrap();
    let id = make(&mut db, case.spec, "t");
    flag::set_sticky(&mut db, id, true).unwrap();
    flag::set_color(&mut db, id, Some("#f8bbd0")).unwrap();
    let key = Key::new(case.spec.meta.id, id);

    let mut e = Engine::new();
    e.refresh().unwrap();
    assert_eq!(e.item(&key).unwrap().color.as_deref(), Some("#f8bbd0"));
    e.set_color(&key, Some("#90caf9")).unwrap();
    assert_eq!(e.item(&key).unwrap().color.as_deref(), Some("#90caf9"));
    assert_eq!(e.item(&key).unwrap().app_color.as_deref(), Some("#f8bbd0"));
    e.set_color(&key, None).unwrap();
    assert_eq!(
        e.item(&key).unwrap().color.as_deref(),
        Some("#f8bbd0"),
        "back to the app's"
    );
    assert_eq!(
        flag::color(&db.get(id).unwrap()),
        Some("#f8bbd0"),
        "the app's is never changed"
    );
}

#[test]
fn what_sticky_kept_is_forgotten_when_the_app_unflags_the_item() {
    let sb = Sandbox::new("forget");
    let case = &cases()[0];
    let path = link_to(&sb, case);
    let mut db = Db::open(&path, case.schema).unwrap();
    let id = make(&mut db, case.spec, "t");
    flag::set_sticky(&mut db, id, true).unwrap();
    let key = Key::new(case.spec.meta.id, id);

    let mut e = Engine::new();
    e.refresh().unwrap();
    e.set_show_on_desktop(&key, false).unwrap();
    e.set_color(&key, Some("#90caf9")).unwrap();
    assert_eq!(store::load_local().unwrap().items.len(), 1);

    // The app stops it being a sticky, then makes it one again: afresh, with the defaults.
    flag::set_sticky(&mut db, id, false).unwrap();
    e.refresh().unwrap();
    assert!(
        store::load_local().unwrap().items.is_empty(),
        "forgotten with the flag"
    );
    flag::set_sticky(&mut db, id, true).unwrap();
    e.refresh().unwrap();
    let item = e.item(&key).unwrap();
    assert!(item.show_on_desktop && item.color.is_none());

    // Switching an app off keeps what Sticky kept (it is only hidden)...
    e.set_show_on_desktop(&key, false).unwrap();
    e.set_linked(case.spec.meta.id, false).unwrap();
    assert_eq!(store::load_local().unwrap().items.len(), 1);
    assert!(e.items().is_empty());
    assert!(
        flag::is_sticky(&db.get(id).unwrap()),
        "the app's data is untouched"
    );
}

#[test]
fn switching_an_app_off_hides_its_stickies_without_touching_them() {
    let sb = Sandbox::new("off");
    let case = &cases()[1];
    let path = link_to(&sb, case);
    let mut db = Db::open(&path, case.schema).unwrap();
    let id = make(&mut db, case.spec, "t");
    flag::set_sticky(&mut db, id, true).unwrap();

    let mut e = Engine::new();
    e.refresh().unwrap();
    assert_eq!(e.items().len(), 1);
    let changes = e.set_linked(case.spec.meta.id, false).unwrap();
    assert_eq!(changes.removed.len(), 1);
    assert!(e.items().is_empty());
    assert!(
        flag::is_sticky(&db.get(id).unwrap()),
        "still a sticky in its app"
    );
    assert_eq!(
        e.set_linked(case.spec.meta.id, true).unwrap().added.len(),
        1
    );
}

#[test]
fn an_app_without_a_database_yet_is_skipped_not_created() {
    let sb = Sandbox::new("nodb");
    let case = &cases()[0];
    let path = link_to(&sb, case);
    let mut e = Engine::new();
    assert!(e.refresh().unwrap().is_empty());
    assert!(!path.exists(), "Sticky must not create the app's database");
}

#[test]
fn the_watcher_notices_a_change_by_looking_at_files_only() {
    let sb = Sandbox::new("watch");
    let case = &cases()[0];
    let path = link_to(&sb, case);
    let mut db = Db::open(&path, case.schema).unwrap();
    let id = make(&mut db, case.spec, "t");

    let mut w = Watcher::new();
    assert!(w.changed(), "the first look says yes");
    assert!(!w.changed(), "and then nothing, while nothing happens");
    assert!(!w.changed());
    flag::set_sticky(&mut db, id, true).unwrap();
    assert!(w.changed(), "an app's write is noticed");
    assert!(!w.changed());
    let mut other = store::link_for(&notes_schema::STICKY);
    other.app_id = "org.example.Other".into();
    store::set_enabled(
        &Link {
            enabled: true,
            ..other
        },
        false,
    )
    .unwrap();
    assert!(w.changed(), "a registry change is noticed");
    assert!(!w.changed());
    let mut local = store::load_local().unwrap();
    local.set_show_on_desktop("x", 1, false);
    store::save_local(&local).unwrap();
    assert!(w.changed(), "so is a change of what Sticky kept");
    assert!(!w.changed());
}
