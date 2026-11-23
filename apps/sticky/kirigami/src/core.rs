//! Sticky Notes' state and every action, with no Qt in it: the bridge in `backend.rs` only
//! forwards. Lists come out as JSON that QML reads directly.
//!
//! Behaviour is the engine's (`sticky-engine`); what lives here is what a screen needs on top of
//! it: the single-instance rule, which stickies get a window, the paper and ink colours, the
//! palette and the Preferences.

use std::fs::File;

use organiser_core::AppMeta;
use organiser_core::settings::{DEFAULT_STICKY_PALETTE, Settings};
use organiser_sticky::{autostart, discover};
use serde_json::{Value, json};
use sticky_engine::{Changes, Engine, Key, StickyItem, Watcher};

pub const META: AppMeta = AppMeta {
    id: organiser_sticky::STICKY_ID,
    slug: "sticky",
    name: "Sticky Notes",
    comment: "Keep notes, tasks and favourites on your desktop",
    categories: &["Office", "Utility"],
};

/// The paper of a sticky when nothing else parses.
const FALLBACK_PAPER: &str = "#fff59d";

/// `#rrggbb` (the `#` optional, any case) as the canonical lower-case `#rrggbb`.
pub fn parse_hex(s: &str) -> Option<String> {
    let h = s.trim().trim_start_matches('#');
    (h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| format!("#{}", h.to_ascii_lowercase()))
}

/// The paper for an optional stored colour; `default` is for a sticky with none.
pub fn paper(stored: Option<&str>, default: &str) -> String {
    stored
        .and_then(parse_hex)
        .or_else(|| parse_hex(default))
        .unwrap_or_else(|| FALLBACK_PAPER.into())
}

/// Dark ink on light paper, light ink on dark paper.
pub fn ink(paper: &str) -> &'static str {
    let n = u32::from_str_radix(paper.trim_start_matches('#'), 16).unwrap_or(0xff_f5_9d);
    let ch = |shift: u32| f64::from((n >> shift) & 0xff) / 255.0;
    let lum = 0.2126 * ch(16) + 0.7152 * ch(8) + 0.0722 * ch(0);
    if lum > 0.5 { "#2b2b2b" } else { "#f5f5f5" }
}

/// What to call a palette colour: its name if it is one of the presets, else its hex code.
pub fn color_name(hex: &str) -> String {
    match hex.to_ascii_lowercase().as_str() {
        "#fff59d" => "Yellow".into(),
        "#c5e1a5" => "Green".into(),
        "#90caf9" => "Blue".into(),
        "#f8bbd0" => "Pink".into(),
        "#ffcc80" => "Orange".into(),
        other => other.into(),
    }
}

/// `app-id#id`: how QML names a sticky.
pub fn key_string(k: &Key) -> String {
    format!("{}#{}", k.app_id, k.id)
}

pub fn parse_key(s: &str) -> Option<Key> {
    let (app, id) = s.rsplit_once('#')?;
    Some(Key::new(app, id.parse().ok()?))
}

pub struct Core {
    engine: Engine,
    watcher: Watcher,
    pub settings: Settings,
    pub autostart: bool,
    /// Only the first Sticky process opens sticky windows; a later one (started for the main
    /// window while another runs) shows the cards alone, so nothing is shown twice.
    owner: bool,
    background: bool,
    /// What the person should be told after the last action.
    pub notice: Option<String>,
    _lock: Option<File>,
}

/// The single-instance lock, held for the life of the process.
fn take_lock() -> Option<File> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("sticky-kirigami.lock"))
        .ok()?;
    f.try_lock().ok().map(|()| f)
}

impl Core {
    /// Start: take the instance lock, read the settings and the stickies.
    pub fn open(background: bool) -> Self {
        let lock = take_lock();
        let mut core = Self::with(background, lock.is_some());
        core._lock = lock;
        core
    }

    /// As [`Core::open`] with the instance decision made by the caller.
    pub fn with(background: bool, owner: bool) -> Self {
        let loaded = Settings::load();
        let mut core = Self {
            engine: Engine::new(),
            watcher: Watcher::new(),
            settings: loaded.settings,
            autostart: autostart::is_enabled(),
            owner,
            background,
            notice: loaded.problem,
            _lock: None,
        };
        core.refresh();
        core
    }

    fn fail(&mut self, e: impl std::fmt::Display) {
        self.notice = Some(e.to_string());
    }

    /// Read the registry and the flagged items again.
    pub fn refresh(&mut self) -> Changes {
        match self.engine.refresh() {
            Ok(c) => c,
            Err(e) => {
                self.fail(e);
                Changes::default()
            }
        }
    }

    /// Look at the watched files' times; when one changed, refresh. True when what is shown
    /// differs now.
    pub fn poll(&mut self) -> bool {
        self.watcher.changed() && !self.refresh().is_empty()
    }

    fn default_color(&self) -> &str {
        &self.settings.sticky.default_color
    }

    fn item_json(&self, i: &StickyItem) -> Value {
        let paper = paper(i.color.as_deref(), self.default_color());
        json!({
            "key": key_string(&i.key),
            "appId": i.key.app_id,
            "id": i.key.id,
            "appName": i.app_name,
            "title": i.title,
            "body": i.body,
            "paper": paper,
            "ink": ink(&paper),
            "colored": i.color.is_some(),
            "desktop": i.show_on_desktop,
        })
    }

    /// Every sticky, for the main window's cards.
    pub fn stickies_json(&self) -> String {
        let all: Vec<Value> = self
            .engine
            .items()
            .iter()
            .map(|i| self.item_json(i))
            .collect();
        Value::Array(all).to_string()
    }

    /// One sticky, `{}` when it is gone.
    pub fn sticky_json(&self, key: &str) -> String {
        parse_key(key)
            .and_then(|k| self.engine.item(&k).map(|i| self.item_json(i)))
            .unwrap_or_else(|| json!({}))
            .to_string()
    }

    /// The stickies that have a window on the desktop (none for a second process).
    pub fn desktop_keys(&self) -> Vec<String> {
        if !self.owner {
            return Vec::new();
        }
        self.engine
            .items()
            .iter()
            .filter(|i| i.show_on_desktop)
            .map(|i| key_string(&i.key))
            .collect()
    }

    /// Whether the process has nothing left to show and should end: a background start that is
    /// not the first Sticky, or no sticky window and no main window.
    pub fn should_exit(&self, manager_open: bool) -> bool {
        (self.background && !self.owner) || (self.desktop_keys().is_empty() && !manager_open)
    }

    /// Whether the main window opens at start (not for `--background`).
    pub fn opens_manager(&self) -> bool {
        !self.background
    }

    pub fn links_json(&self) -> String {
        let links: Vec<Value> = self
            .engine
            .links()
            .links
            .iter()
            .map(|l| {
                json!({
                    "appId": l.app_id,
                    "slug": l.slug,
                    "title": l.title(),
                    "dbPath": l.db_path,
                    "enabled": l.enabled,
                })
            })
            .collect();
        Value::Array(links).to_string()
    }

    pub fn palette_json(&self) -> String {
        let p: Vec<Value> = self
            .settings
            .sticky
            .palette
            .iter()
            .map(|c| json!({ "hex": c, "name": color_name(c), "ink": ink(c) }))
            .collect();
        Value::Array(p).to_string()
    }

    pub fn default_color_hex(&self) -> String {
        paper(None, self.default_color())
    }

    fn key(&self, key: &str) -> Result<Key, String> {
        parse_key(key).ok_or_else(|| format!("not a sticky: {key}"))
    }

    /// Run an engine action; a failure becomes the notice.
    fn act(&mut self, r: Result<(), String>) -> bool {
        match r {
            Ok(()) => true,
            Err(e) => {
                self.fail(e);
                false
            }
        }
    }

    pub fn set_title(&mut self, key: &str, title: &str) -> bool {
        let r = self.key(key).and_then(|k| self.engine.set_title(&k, title));
        self.act(r)
    }

    pub fn set_body(&mut self, key: &str, body: &str) -> bool {
        let body = body.trim_end();
        let r = self.key(key).and_then(|k| self.engine.set_body(&k, body));
        self.act(r)
    }

    /// Paint a sticky; an empty or invalid colour is an error, `None` clears it.
    pub fn set_color(&mut self, key: &str, hex: Option<&str>) -> bool {
        let r = (|| {
            let k = self.key(key)?;
            let hex = match hex {
                Some(h) => {
                    Some(parse_hex(h).ok_or_else(|| format!("{h} is not a #rrggbb colour"))?)
                }
                None => None,
            };
            self.engine.set_color(&k, hex.as_deref())
        })();
        self.act(r)
    }

    /// Show a sticky on the desktop or only in the main window.
    pub fn set_desktop(&mut self, key: &str, on: bool) -> bool {
        let r = self
            .key(key)
            .and_then(|k| self.engine.set_show_on_desktop(&k, on));
        self.act(r)
    }

    pub fn set_linked(&mut self, app_id: &str, on: bool) -> bool {
        match self.engine.set_linked(app_id, on) {
            Ok(_) => true,
            Err(e) => {
                self.fail(e);
                false
            }
        }
    }

    /// Open a linked app on its create page.
    pub fn new_in(&mut self, app_id: &str) -> bool {
        let slug = self
            .engine
            .links()
            .get(app_id)
            .map(|l| l.slug.clone())
            .unwrap_or_default();
        let r = discover::open_new(&slug, app_id);
        self.act(r)
    }

    pub fn set_autostart(&mut self, on: bool) -> bool {
        match autostart::set_enabled(on) {
            Ok(()) => {
                self.autostart = on;
                true
            }
            Err(e) => {
                self.fail(e);
                false
            }
        }
    }

    fn save_settings(&mut self) -> bool {
        self.settings = self.settings.clone().normalized();
        let r = self.settings.save();
        self.act(r)
    }

    pub fn set_default_color(&mut self, hex: &str) -> bool {
        match parse_hex(hex) {
            Some(h) => {
                self.settings.sticky.default_color = h;
                self.save_settings()
            }
            None => self.act(Err(format!("{hex} is not a #rrggbb colour"))),
        }
    }

    pub fn set_palette_color(&mut self, index: usize, hex: &str) -> bool {
        match (parse_hex(hex), self.settings.sticky.palette.get_mut(index)) {
            (Some(h), Some(slot)) => {
                *slot = h;
                self.save_settings()
            }
            (None, _) => self.act(Err(format!("{hex} is not a #rrggbb colour"))),
            _ => false,
        }
    }

    pub fn add_palette_color(&mut self) -> bool {
        self.settings.sticky.palette.push("#e0e0e0".into());
        self.save_settings()
    }

    /// Never empty: a sticky has to be able to take some colour.
    pub fn remove_palette_color(&mut self, index: usize) -> bool {
        let p = &mut self.settings.sticky.palette;
        if p.len() > 1 && index < p.len() {
            p.remove(index);
            return self.save_settings();
        }
        false
    }

    pub fn reset_palette(&mut self) -> bool {
        self.settings.sticky.palette = DEFAULT_STICKY_PALETTE.map(String::from).to_vec();
        self.save_settings()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Mutex;

    use organiser_engine::{Db, NewItem};
    use organiser_sticky::{flag, store};

    #[test]
    fn hex_is_normalised_and_paper_falls_back() {
        assert_eq!(parse_hex("#FF0000").as_deref(), Some("#ff0000"));
        assert_eq!(parse_hex("00ff00").as_deref(), Some("#00ff00"));
        assert_eq!(parse_hex("#ff00"), None);
        assert_eq!(parse_hex("#gg0000"), None);
        assert_eq!(paper(None, "#00ff00"), "#00ff00");
        assert_eq!(paper(Some("nope"), "#00ff00"), "#00ff00");
        assert_eq!(paper(Some("#0000ff"), "#00ff00"), "#0000ff");
        assert_eq!(paper(None, "bad"), FALLBACK_PAPER);
    }

    #[test]
    fn ink_contrasts_with_the_paper() {
        assert_eq!(ink("#fff59d"), "#2b2b2b");
        assert_eq!(ink("#000000"), "#f5f5f5");
    }

    #[test]
    fn colours_have_names_and_keys_round_trip() {
        assert_eq!(color_name("#C5E1A5"), "Green");
        assert_eq!(color_name("#123456"), "#123456");
        let k = Key::new("io.github.vazark.Notes", 42);
        assert_eq!(parse_key(&key_string(&k)), Some(k));
        assert_eq!(parse_key("nonsense"), None);
        assert_eq!(parse_key("app#x"), None);
    }

    /// The registry and settings are read from `XDG_CONFIG_HOME`: one test at a time.
    static ENV: Mutex<()> = Mutex::new(());

    struct Sandbox {
        dir: PathBuf,
        db: Db,
        _guard: std::sync::MutexGuard<'static, ()>,
    }

    impl Sandbox {
        fn new(name: &str) -> Self {
            let guard = ENV.lock().unwrap_or_else(|e| e.into_inner());
            let dir =
                std::env::temp_dir().join(format!("sticky-kirigami-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            // SAFETY: tests hold `ENV` while they use the variable.
            unsafe { std::env::set_var("XDG_CONFIG_HOME", dir.join("config")) };
            let spec = &notes_schema::STICKY;
            let path = dir.join("notes.db");
            let mut link = store::link_for(spec);
            link.db_path = path.to_string_lossy().into_owned();
            store::set_enabled(&link, true).unwrap();
            let db = Db::open(&path, &notes_schema::SCHEMA).unwrap();
            Self {
                dir,
                db,
                _guard: guard,
            }
        }

        /// A flagged note; its key string.
        fn sticky(&mut self, title: &str) -> String {
            let k = &notes_schema::STICKY.kinds[0];
            let id = self
                .db
                .create(NewItem::new(k.kind, title).field(k.body, "text"))
                .unwrap()
                .id;
            flag::set_sticky(&mut self.db, id, true).unwrap();
            key_string(&Key::new(notes_schema::STICKY.meta.id, id))
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn edits_and_the_desktop_choice_reach_the_json() {
        let mut sb = Sandbox::new("edits");
        let key = sb.sticky("first");
        let mut core = Core::with(false, true);
        assert_eq!(core.desktop_keys(), std::slice::from_ref(&key));

        assert!(core.set_title(&key, "renamed"));
        assert!(core.set_body(&key, "line\n\n"));
        assert!(core.set_color(&key, Some("#90CAF9")));
        let v: Value = serde_json::from_str(&core.sticky_json(&key)).unwrap();
        assert_eq!(v["title"], "renamed");
        assert_eq!(v["body"], "line");
        assert_eq!(v["paper"], "#90caf9");
        assert_eq!(v["colored"], true);
        assert_eq!(v["ink"], "#2b2b2b");

        assert!(
            !core.set_color(&key, Some("blue")),
            "a bad colour is refused"
        );
        assert!(core.notice.is_some());
        assert!(core.set_color(&key, None));
        let v: Value = serde_json::from_str(&core.sticky_json(&key)).unwrap();
        assert_eq!(v["colored"], false);
        assert_eq!(v["paper"], core.default_color_hex());

        // Off the desktop: still a card, no window.
        assert!(core.set_desktop(&key, false));
        assert!(core.desktop_keys().is_empty());
        let all: Vec<Value> = serde_json::from_str(&core.stickies_json()).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0]["desktop"], false);
        assert!(core.should_exit(false));
        assert!(!core.should_exit(true));
        assert_eq!(core.sticky_json("gone#1"), "{}");
    }

    #[test]
    fn poll_sees_changes_made_elsewhere() {
        let mut sb = Sandbox::new("poll");
        let mut core = Core::with(false, true);
        assert!(core.desktop_keys().is_empty());
        core.poll(); // the first look
        assert!(!core.poll(), "nothing changed");
        let key = sb.sticky("late");
        assert!(core.poll());
        assert_eq!(core.desktop_keys(), [key]);
    }

    #[test]
    fn only_the_first_process_opens_windows_and_background_ends_when_idle() {
        let mut sb = Sandbox::new("owner");
        sb.sticky("a");
        let second = Core::with(false, false);
        assert!(second.desktop_keys().is_empty());
        assert!(!second.should_exit(true), "its main window keeps it");
        assert!(second.opens_manager());
        let bg_second = Core::with(true, false);
        assert!(bg_second.should_exit(false), "another Sticky shows them");
        assert!(!bg_second.opens_manager());
        let bg = Core::with(true, true);
        assert!(!bg.should_exit(false));
    }

    #[test]
    fn linking_an_app_off_removes_its_stickies() {
        let mut sb = Sandbox::new("linked");
        sb.sticky("a");
        let mut core = Core::with(false, true);
        let links: Vec<Value> = serde_json::from_str(&core.links_json()).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0]["enabled"], true);
        assert!(core.set_linked(notes_schema::STICKY.meta.id, false));
        assert!(core.desktop_keys().is_empty());
        assert!(core.set_linked(notes_schema::STICKY.meta.id, true));
        assert_eq!(core.desktop_keys().len(), 1);
    }

    #[test]
    fn the_palette_keeps_at_least_one_colour_and_checks_hex() {
        let _sb = Sandbox::new("palette");
        let mut core = Core::with(false, true);
        assert!(core.set_palette_color(0, "#112233"));
        assert_eq!(core.settings.sticky.palette[0], "#112233");
        assert!(!core.set_palette_color(0, "zzz"));
        assert!(!core.set_palette_color(99, "#112233"));
        assert!(core.add_palette_color());
        assert_eq!(core.settings.sticky.palette.len(), 6);
        assert!(core.remove_palette_color(5));
        while core.settings.sticky.palette.len() > 1 {
            assert!(core.remove_palette_color(0));
        }
        assert!(!core.remove_palette_color(0), "the last one stays");
        assert!(core.reset_palette());
        assert_eq!(core.settings.sticky.palette.len(), 5);
        assert!(core.set_default_color("#ABCDEF"));
        assert_eq!(core.default_color_hex(), "#abcdef");
        assert!(!core.set_default_color("red"));
        // Saved: a new core reads it back.
        let again = Core::with(false, true);
        assert_eq!(again.default_color_hex(), "#abcdef");
    }
}
