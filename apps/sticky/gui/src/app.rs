//! State, messages and update of the Sticky Notes GUI: one window per sticky, and a main window
//! with every sticky as a card and the Preferences. All behaviour is in `sticky-engine`; this keeps
//! what only a screen needs (windows, the text being typed, which page is open).
//!
//! Nothing is polled. A background thread looks at a few files' modification times (see
//! `sticky_engine::Watcher`) and sends a message only when one changed.

use std::collections::HashMap;
use std::time::Duration;

use iced::futures::{SinkExt, StreamExt};
use iced::widget::text_editor;
use iced::{Size, Subscription, Task, window};
use organiser_core::settings::{DEFAULT_STICKY_PALETTE, Settings};
use organiser_sticky::{Links, autostart};
use organiser_ui::iad::primitives::color_picker::Hsva;
use organiser_ui::iad::primitives::rich_text as rt;
use organiser_ui::iad::{self, Scheme};
use organiser_ui::primitives::scheme::Mode;
use organiser_ui::shell;
use sticky_engine::{Engine, Key, StickyItem, Watcher};

const STICKY_SIZE: Size = Size::new(300.0, 320.0);
/// How often the watcher looks at file times: a few `stat` calls, nothing is read.
const WATCH_EVERY: Duration = Duration::from_millis(700);

#[derive(Debug, Clone)]
pub enum Message {
    SystemTheme(iad::utils::ThemeSettings),
    /// A file Sticky shows from may have changed.
    Changed,
    Opened,
    Closed(window::Id),
    Title(window::Id, String),
    Body(window::Id, text_editor::Action),
    /// A colour chosen from the palette in a sticky's menu.
    ColorText(window::Id, String),
    /// Open Sticky Notes' main window, with every sticky (closing the sticky's menu).
    OpenManager(window::Id),
    /// Open an app (by its app id) on its create page; chosen in a sticky's "+" menu.
    New(window::Id, String),
    /// Show the colour page of a sticky's menu (true) or its first page (false).
    MenuColours(window::Id, bool),
    /// Take the sticky off the desktop: it stays in Sticky Notes' window, and in its app.
    Hide(window::Id),
    /// Show a sticky on the desktop (true) or only in Sticky Notes' window (false).
    Desktop(Key, bool),
    /// Switch a sticky between reading (Markdown drawn) and editing its source.
    ToggleEdit(window::Id),
    Drag(window::Id),
    Resize(window::Id),
    /// Switch a published app on (true) or off (false), by its app id.
    SetLinked(String, bool),
    /// The default paper of stickies that have no colour.
    DefaultText(String),
    DefaultPick(Hsva),
    PaletteColor(usize, Hsva),
    PaletteText(usize, String),
    PaletteAdd,
    PaletteRemove(usize),
    PaletteReset,
    /// The main window's chrome: drag, minimise, maximise, close, resize.
    Window(iad::windows::Action),
    OpenPrefs,
    ClosePrefs,
    /// Choose a page of Preferences where the list stays beside it (a wide window).
    PrefsPage(PrefsPage),
    /// Go into a page from the list of pages (a narrow window).
    PrefsOpen(PrefsPage),
    PrefsBack,
    /// Escape: back to the list of pages, else close Preferences.
    Escape,
    /// A card was pressed: bring its sticky forward.
    Show(Key),
    Quit,
    Autostart(bool),
    DismissToast,
}

/// The pages of Preferences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefsPage {
    Apps,
    Colours,
    General,
}

impl PrefsPage {
    pub const ALL: [PrefsPage; 3] = [Self::Apps, Self::Colours, Self::General];

    pub fn title(self) -> &'static str {
        match self {
            Self::Apps => "Linked apps",
            Self::Colours => "Colours",
            Self::General => "General",
        }
    }

    pub fn icon(self) -> iad::prelude::Icon {
        use iad::prelude::Icon;
        match self {
            Self::Apps => Icon::Note,
            Self::Colours => Icon::Palette,
            Self::General => Icon::Settings,
        }
    }
}

/// What the screen keeps of one sticky: its window and what is being typed.
pub struct Sticky {
    pub key: Key,
    /// Its window (only the first Sticky process opens them).
    pub win: Option<window::Id>,
    pub app_name: String,
    pub title: String,
    pub body: text_editor::Content,
    /// The body drawn as Markdown, kept so a redraw does not parse it again.
    pub doc: Vec<rt::Block>,
    pub editing: bool,
    /// On the desktop as a window, or only in Sticky Notes' main window.
    pub show_on_desktop: bool,
    pub color: Option<String>,
    /// What the colour popover's entry shows, kept so typing is not interrupted.
    pub color_text: String,
    /// The "⋮" menu is on its colour page.
    pub menu_colours: bool,
    /// Flips each time the menu should close: the popover cannot be closed from outside, but it
    /// is rebuilt (and so closed) when the shape of the view changes.
    pub menu_flip: bool,
}

impl Sticky {
    fn new(item: &StickyItem) -> Self {
        let mut s = Self {
            key: item.key.clone(),
            win: None,
            app_name: item.app_name.clone(),
            title: String::new(),
            body: text_editor::Content::new(),
            doc: Vec::new(),
            editing: false,
            show_on_desktop: true,
            color: None,
            color_text: String::new(),
            menu_colours: false,
            menu_flip: false,
        };
        s.load(item);
        s
    }

    /// Show what the database holds, leaving what is typed alone when it already matches.
    fn load(&mut self, item: &StickyItem) {
        self.title = item.title.clone();
        if self.body.text().trim_end() != item.body {
            self.body = text_editor::Content::with_text(&item.body);
        }
        self.doc = organiser_ui::components::rich::render(&item.body);
        self.show_on_desktop = item.show_on_desktop;
        self.color = item.color.clone();
        self.color_text = item.color.clone().unwrap_or_default();
    }

    fn open_window(&mut self) -> Task<Message> {
        let (win, open) = window::open(window::Settings {
            size: STICKY_SIZE,
            min_size: Some(Size::new(200.0, 160.0)),
            // The toolkit's own settings: transparent, so the corners can be rounded like the
            // main window's.
            ..iad::windows::settings_for(&self.key.app_id)
        });
        self.win = Some(win);
        open.map(|_| Message::Opened)
    }
}

pub struct App {
    engine: Engine,
    /// Only the first Sticky process opens sticky windows; a later one (started for the main
    /// window while another runs) shows the cards alone, so nothing is shown twice.
    owner: bool,
    background: bool,
    pub stickies: Vec<Sticky>,
    pub manager: Option<window::Id>,
    pub prefs_open: bool,
    pub prefs_page: PrefsPage,
    /// Narrow window: a page is open (otherwise the list of pages shows, as the first page).
    pub prefs_page_open: bool,
    pub autostart: bool,
    pub settings: Settings,
    /// What each palette entry's entry shows, so typing is not interrupted.
    pub palette_texts: HashMap<usize, String>,
    pub default_text: String,
    pub toast: Option<String>,
    mode: Mode,
    desktop_dark: bool,
}

/// The single-instance lock, held for the life of the process.
fn take_lock() -> Option<std::fs::File> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("sticky-gui.lock"))
        .ok()?;
    f.try_lock().ok().map(|()| f)
}

/// A message whenever the watcher says a watched file changed.
fn watch() -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(1, async |mut out| {
        let (tx, mut rx) = iced::futures::channel::mpsc::unbounded::<()>();
        std::thread::spawn(move || {
            let mut watcher = Watcher::new();
            loop {
                if watcher.changed() && tx.unbounded_send(()).is_err() {
                    break;
                }
                std::thread::sleep(WATCH_EVERY);
            }
        });
        while rx.next().await.is_some() {
            if out.send(Message::Changed).await.is_err() {
                break;
            }
        }
    })
}

impl App {
    pub fn boot(background: bool) -> (Self, Task<Message>) {
        let lock = take_lock();
        let owner = lock.is_some();
        // Kept for the life of the process.
        std::mem::forget(lock);
        let loaded = Settings::load();
        let mut app = Self {
            engine: Engine::new(),
            owner,
            background,
            stickies: Vec::new(),
            manager: None,
            prefs_open: false,
            prefs_page: PrefsPage::Apps,
            prefs_page_open: false,
            autostart: autostart::is_enabled(),
            default_text: loaded.settings.sticky.default_color.clone(),
            palette_texts: HashMap::new(),
            mode: Mode::from(loaded.settings.appearance.theme),
            settings: loaded.settings,
            toast: loaded.problem,
            desktop_dark: false,
        };
        if background && !owner {
            // Another Sticky is running and will pick up what changed.
            return (app, iced::exit());
        }
        let mut tasks = vec![app.sync()];
        if background && app.shown() == 0 {
            return (app, iced::exit());
        }
        if !background {
            tasks.push(app.open_manager());
        }
        // Development switch: open Preferences on a page (`apps`, `colours`, `general`; `list` for
        // the narrow window's list of pages).
        if let Ok(p) = std::env::var("STICKY_GUI_PREFS") {
            app.prefs_open = true;
            app.prefs_page_open = p != "list";
            app.prefs_page = match p.as_str() {
                "colours" => PrefsPage::Colours,
                "general" => PrefsPage::General,
                _ => PrefsPage::Apps,
            };
        }
        // Development switch, for a picture of a window: `ICED_ADAPTIVE_DESIGN_SCREENSHOT=path.png`
        // with `STICKY_GUI_SHOT=main` for the main window (a sticky otherwise), then quit.
        let dev = iad::dev::Switches::from_env();
        if dev.screenshot.is_some() {
            let target = if std::env::var("STICKY_GUI_SHOT").as_deref() == Ok("main") {
                app.manager
            } else {
                app.stickies.iter().find_map(|s| s.win)
            };
            if let Some(id) = target
                && let Some(t) = dev.capture(id)
            {
                tasks.push(t.map(|_| Message::Quit));
            }
        }
        (app, Task::batch(tasks))
    }

    pub fn title(&self, id: window::Id) -> String {
        match self.stickies.iter().find(|s| s.win == Some(id)) {
            Some(s) if !s.title.trim().is_empty() => s.title.clone(),
            Some(_) => "Sticky".into(),
            None => "Sticky Notes".into(),
        }
    }

    pub fn theme(&self, id: window::Id) -> iad::Theme {
        // A sticky is pastel paper: always the light theme, so its text reads on it.
        if self.stickies.iter().any(|s| s.win == Some(id)) {
            return shell::theme(false);
        }
        shell::theme(self.mode.is_dark(self.desktop_dark))
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iad::utils::follow_theme().map(Message::SystemTheme),
            window::close_events().map(Message::Closed),
            Subscription::run(watch),
        ])
    }

    fn fail(&mut self, e: impl std::fmt::Display) {
        self.toast = Some(e.to_string());
    }

    fn sticky_mut(&mut self, id: window::Id) -> Option<&mut Sticky> {
        self.stickies.iter_mut().find(|s| s.win == Some(id))
    }

    fn key_of(&self, id: window::Id) -> Option<Key> {
        self.stickies
            .iter()
            .find(|s| s.win == Some(id))
            .map(|s| s.key.clone())
    }

    /// The registry as the engine last read it, for the Preferences.
    pub fn links(&self) -> &Links {
        self.engine.links()
    }

    /// Ask the compositor to bring a window forward: `gain_focus` where the windowing system lets
    /// an app do that (X11), and an xdg-activation request (`request_user_attention`) on Wayland,
    /// where the compositor decides whether to raise it or only mark it as wanting attention.
    fn raise(id: window::Id) -> Task<Message> {
        Task::batch([
            window::gain_focus(id),
            window::request_user_attention(id, Some(window::UserAttention::Informational)),
        ])
    }

    /// Show the main window: open it, or bring the open one forward.
    fn open_manager(&mut self) -> Task<Message> {
        if let Some(id) = self.manager {
            return Self::raise(id);
        }
        let (id, open) = window::open(window::Settings {
            min_size: Some(Size::new(360.0, 300.0)),
            ..shell::window(&crate::META, Size::new(900.0, 640.0))
        });
        self.manager = Some(id);
        open.map(|_| Message::Opened)
    }

    /// Ask the engine to look again and make the windows match.
    fn sync(&mut self) -> Task<Message> {
        match self.engine.refresh() {
            Ok(changes) => self.reconcile(changes),
            Err(e) => {
                self.fail(e);
                Task::none()
            }
        }
    }

    fn reconcile(&mut self, changes: sticky_engine::Changes) -> Task<Message> {
        let mut tasks = Vec::new();
        for key in &changes.removed {
            if let Some(i) = self.stickies.iter().position(|s| &s.key == key) {
                let gone = self.stickies.remove(i);
                if let Some(w) = gone.win {
                    tasks.push(window::close(w));
                }
            }
        }
        for key in &changes.updated {
            let (Some(item), Some(s)) = (
                self.engine.item(key),
                self.stickies.iter_mut().find(|s| &s.key == key),
            ) else {
                continue;
            };
            s.load(item);
            // Hidden from, or shown on, the desktop from somewhere else (the command line, a
            // second Sticky window).
            if self.owner {
                match (s.show_on_desktop, s.win) {
                    (true, None) => tasks.push(s.open_window()),
                    (false, Some(w)) => {
                        s.win = None;
                        tasks.push(window::close(w));
                    }
                    _ => {}
                }
            }
        }
        for key in &changes.added {
            let Some(item) = self.engine.item(key) else {
                continue;
            };
            let mut s = Sticky::new(item);
            if self.owner && s.show_on_desktop {
                tasks.push(s.open_window());
            }
            self.stickies.push(s);
        }
        Task::batch(tasks)
    }

    /// How many sticky windows are open.
    pub fn shown(&self) -> usize {
        self.stickies.iter().filter(|s| s.win.is_some()).count()
    }

    /// The paper of a sticky with no colour of its own.
    pub fn default_color(&self) -> &str {
        &self.settings.sticky.default_color
    }

    fn save_settings(&mut self) {
        self.settings = self.settings.clone().normalized();
        if let Err(e) = self.settings.save() {
            self.fail(e);
        }
    }

    /// Show or hide a sticky on the desktop: Sticky Notes' own choice, kept by Sticky; the item in
    /// its app is not touched and stays a sticky. A hidden sticky stays in the main window.
    fn set_desktop(&mut self, key: &Key, on: bool) -> Task<Message> {
        if let Err(e) = self.engine.set_show_on_desktop(key, on) {
            self.fail(e);
            return Task::none();
        }
        let owner = self.owner;
        let mut task = Task::none();
        if let Some(s) = self.stickies.iter_mut().find(|s| &s.key == key) {
            s.show_on_desktop = on;
            if owner {
                match (on, s.win) {
                    (true, None) => task = s.open_window(),
                    (false, Some(w)) => {
                        s.win = None;
                        task = window::close(w);
                    }
                    _ => {}
                }
            }
        }
        if self.shown() == 0 && self.manager.is_none() {
            return iced::exit();
        }
        task
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SystemTheme(s) => self.desktop_dark = s.scheme == Scheme::Dark,
            Message::Opened => {}
            Message::Changed => {
                let t = self.sync();
                if self.background && self.shown() == 0 && self.manager.is_none() {
                    return iced::exit();
                }
                return t;
            }
            Message::Closed(id) => {
                if self.manager == Some(id) {
                    self.manager = None;
                    if self.shown() == 0 {
                        return iced::exit();
                    }
                    return Task::none();
                }
                // Closing a sticky's window takes it off the desktop, like its ✕.
                if let Some(key) = self.key_of(id) {
                    if let Some(s) = self.sticky_mut(id) {
                        s.win = None;
                    }
                    return self.set_desktop(&key, false);
                }
            }
            Message::Title(id, t) => {
                let Some(key) = self.key_of(id) else {
                    return Task::none();
                };
                if let Some(s) = self.sticky_mut(id) {
                    s.title = t.clone();
                }
                if let Err(e) = self.engine.set_title(&key, &t) {
                    self.fail(e);
                }
            }
            Message::Body(id, action) => {
                let Some(key) = self.key_of(id) else {
                    return Task::none();
                };
                let Some(s) = self.sticky_mut(id) else {
                    return Task::none();
                };
                let edit = action.is_edit();
                s.body.perform(action);
                if edit {
                    let text = s.body.text().trim_end().to_string();
                    s.doc = organiser_ui::components::rich::render(&text);
                    if let Err(e) = self.engine.set_body(&key, &text) {
                        self.fail(e);
                    }
                }
            }
            Message::ColorText(id, text) => {
                let parsed = Hsva::parse(&text).map(hex_of);
                if let Some(s) = self.sticky_mut(id) {
                    s.color_text = text;
                }
                if let Some(h) = parsed {
                    self.set_color(id, h);
                }
            }
            Message::OpenManager(id) => {
                self.close_menu(id);
                return self.open_manager();
            }
            Message::New(id, app_id) => {
                self.close_menu(id);
                let slug = self
                    .links()
                    .get(&app_id)
                    .map(|l| l.slug.clone())
                    .unwrap_or_default();
                if let Err(e) = organiser_sticky::discover::open_new(&slug, &app_id) {
                    self.fail(e);
                }
            }
            Message::MenuColours(id, on) => {
                if let Some(s) = self.sticky_mut(id) {
                    s.menu_colours = on;
                }
            }
            Message::Hide(id) => {
                if let Some(key) = self.key_of(id) {
                    return self.set_desktop(&key, false);
                }
            }
            Message::Desktop(key, on) => return self.set_desktop(&key, on),
            Message::ToggleEdit(id) => {
                if let Some(s) = self.sticky_mut(id) {
                    s.editing = !s.editing;
                }
            }
            Message::Drag(id) => return window::drag(id),
            Message::Resize(id) => return window::drag_resize(id, window::Direction::SouthEast),
            Message::SetLinked(app_id, on) => match self.engine.set_linked(&app_id, on) {
                Ok(changes) => return self.reconcile(changes),
                Err(e) => self.fail(e),
            },
            Message::DefaultText(text) => {
                let parsed = Hsva::parse(&text).map(hex_of);
                self.default_text = text;
                if let Some(h) = parsed {
                    self.settings.sticky.default_color = h;
                    self.save_settings();
                }
            }
            Message::DefaultPick(h) => {
                self.settings.sticky.default_color = hex_of(h);
                self.default_text = hex_of(h);
                self.save_settings();
            }
            Message::PaletteColor(i, h) => {
                if let Some(c) = self.settings.sticky.palette.get_mut(i) {
                    *c = hex_of(h);
                    self.palette_texts.insert(i, hex_of(h));
                    self.save_settings();
                }
            }
            Message::PaletteText(i, text) => {
                let parsed = Hsva::parse(&text).map(hex_of);
                self.palette_texts.insert(i, text);
                if let (Some(c), Some(slot)) = (parsed, self.settings.sticky.palette.get_mut(i)) {
                    *slot = c;
                    self.save_settings();
                }
            }
            Message::PaletteAdd => {
                self.settings.sticky.palette.push("#e0e0e0".into());
                self.save_settings();
            }
            Message::PaletteRemove(i) => {
                // Never empty: a sticky has to be able to take some colour.
                let p = &mut self.settings.sticky.palette;
                if p.len() > 1 && i < p.len() {
                    p.remove(i);
                    self.palette_texts.clear();
                    self.save_settings();
                }
            }
            Message::PaletteReset => {
                self.settings.sticky.palette = DEFAULT_STICKY_PALETTE.map(String::from).to_vec();
                self.palette_texts.clear();
                self.save_settings();
            }
            Message::Window(a) => {
                if let Some(id) = self.manager {
                    return a.perform(id);
                }
            }
            Message::OpenPrefs => {
                self.prefs_open = true;
                self.prefs_page_open = false;
            }
            Message::ClosePrefs => self.prefs_open = false,
            Message::PrefsPage(p) => self.prefs_page = p,
            Message::PrefsOpen(p) => {
                self.prefs_page = p;
                self.prefs_page_open = true;
            }
            Message::PrefsBack => self.prefs_page_open = false,
            Message::Escape => {
                if self.prefs_open && !std::mem::take(&mut self.prefs_page_open) {
                    self.prefs_open = false;
                }
            }
            Message::Quit => return iced::exit(),
            Message::Show(key) => {
                if let Some(w) = self
                    .stickies
                    .iter()
                    .find(|s| s.key == key)
                    .and_then(|s| s.win)
                {
                    return Self::raise(w);
                }
                // Hidden from the desktop: pressing its card puts it back.
                return self.set_desktop(&key, true);
            }
            Message::Autostart(on) => match autostart::set_enabled(on) {
                Ok(()) => self.autostart = on,
                Err(e) => self.fail(e),
            },
            Message::DismissToast => self.toast = None,
        }
        Task::none()
    }

    /// Close a sticky's menu and bring it back to its first page.
    fn close_menu(&mut self, win: window::Id) {
        if let Some(s) = self.sticky_mut(win) {
            s.menu_colours = false;
            s.menu_flip = !s.menu_flip;
        }
    }

    /// Paint a sticky and write it back.
    fn set_color(&mut self, win: window::Id, hex: String) {
        let Some(key) = self.key_of(win) else {
            return;
        };
        if let Some(s) = self.sticky_mut(win) {
            s.color = Some(hex.clone());
            s.color_text = hex.clone();
        }
        if let Err(e) = self.engine.set_color(&key, Some(&hex)) {
            self.fail(e);
        }
    }
}

fn hex_of(h: Hsva) -> String {
    let [r, g, b] = h.to_rgb8();
    format!("#{r:02x}{g:02x}{b:02x}")
}
