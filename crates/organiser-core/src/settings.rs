//! The settings every organiser app shares, in one file: `$XDG_CONFIG_HOME/organiser/settings.toml`
//! (else `~/.config/organiser/settings.toml`). Global choices (the theme, the colours new tags
//! take) and each app's own defaults sit side by side, so editing them in one app changes them
//! in all.
//!
//! Reading is forgiving: a missing file, a missing key or a value that makes no sense falls back
//! to the default for that one thing, and unknown keys are kept out of the way. A file that
//! cannot be read at all gives the defaults and says why (see [`Loaded`]); it is never replaced
//! until the person changes a setting.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Light or dark, or whatever the desktop is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    #[default]
    Follow,
    Light,
    Dark,
}

impl ThemeChoice {
    pub const ALL: [ThemeChoice; 3] = [Self::Follow, Self::Light, Self::Dark];
}

impl std::fmt::Display for ThemeChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Follow => "Follow the desktop",
            Self::Light => "Light",
            Self::Dark => "Dark",
        })
    }
}

/// What Todo opens on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StartView {
    /// Every task a note, in a grid of cards.
    #[default]
    Notes,
    Board,
    List,
    /// The Eisenhower matrix: four boxes, each a compact list.
    Quadrants,
}

impl StartView {
    pub const ALL: [StartView; 4] = [Self::Notes, Self::Board, Self::List, Self::Quadrants];
}

impl std::fmt::Display for StartView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Notes => "Notes",
            Self::Board => "Kanban board",
            Self::List => "List",
            Self::Quadrants => "Quadrants",
        })
    }
}

/// The colours new tags are drawn from, until the default palette is changed.
pub const DEFAULT_PALETTE: [&str; 10] = [
    "#e66a6a", "#f0a35e", "#e0c040", "#7fc97f", "#5cc2b8", "#6aa5e8", "#8e99f0", "#a98be0",
    "#e88fc0", "#9aa0a6",
];

/// The paper colours Sticky Notes offers, until its palette is changed.
pub const DEFAULT_STICKY_PALETTE: [&str; 5] =
    ["#fff59d", "#c5e1a5", "#90caf9", "#f8bbd0", "#ffcc80"];

/// How long a note stays in the trash, unless changed.
pub const DEFAULT_TRASH_DAYS: u32 = 30;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub theme: ThemeChoice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TagSettings {
    /// `#rrggbb` colours; a new tag gets one by its name.
    pub palette: Vec<String>,
}

impl Default for TagSettings {
    fn default() -> Self {
        Self {
            palette: DEFAULT_PALETTE.map(String::from).to_vec(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TodoSettings {
    pub start_view: StartView,
    /// Start with finished tasks shown.
    pub show_closed: bool,
    /// Open a task's details in a side drawer on wide windows. Off, or on a narrow window, the
    /// details are a full page that fades in.
    pub details_in_drawer: bool,
    /// List subtasks as entries of their own, in every view. Off, only top-level tasks are
    /// listed (their subtasks are still counted and shown in their details).
    pub show_subtasks: bool,
    /// In the views of every task, group them by epic after the pinned ones, the tasks in no epic
    /// last under "Unassigned".
    pub group_by_epic: bool,
    /// On an epic's page, group its tasks by section.
    pub group_by_section: bool,
}

impl Default for TodoSettings {
    fn default() -> Self {
        Self {
            start_view: StartView::default(),
            show_closed: false,
            details_in_drawer: true,
            show_subtasks: true,
            group_by_epic: true,
            group_by_section: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NotesSettings {
    /// Days a note stays in the trash before it is deleted for good.
    pub trash_days: u32,
    /// Show the notes as a list instead of cards.
    pub list_view: bool,
    /// In the views of every note (All, Favorites, Archive, Trash), group them by folder.
    pub group_by_folder: bool,
    /// On a folder's page, group its notes by section.
    pub group_by_section: bool,
}

impl Default for NotesSettings {
    fn default() -> Self {
        Self {
            trash_days: DEFAULT_TRASH_DAYS,
            list_view: false,
            group_by_folder: true,
            group_by_section: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StickySettings {
    /// `#rrggbb` paper colours a sticky can take.
    pub palette: Vec<String>,
    /// The paper of a sticky that has no colour of its own.
    pub default_color: String,
}

impl Default for StickySettings {
    fn default() -> Self {
        Self {
            palette: DEFAULT_STICKY_PALETTE.map(String::from).to_vec(),
            default_color: DEFAULT_STICKY_PALETTE[0].into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RecipesSettings {
    /// One compact line per recipe instead of cards.
    pub compact: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub appearance: Appearance,
    pub tags: TagSettings,
    pub todo: TodoSettings,
    pub notes: NotesSettings,
    pub recipes: RecipesSettings,
    pub sticky: StickySettings,
}

/// What reading the settings found: the settings to use, and why they are not what the file says,
/// if they are not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    pub settings: Settings,
    /// The file could not be read as settings, so the defaults are in use.
    pub problem: Option<String>,
}

/// Whether `s` is `#rrggbb`.
fn is_hex(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

impl Settings {
    /// Where the shared file lives.
    pub fn path() -> PathBuf {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".config")))
            .unwrap_or_else(|| PathBuf::from(".config"));
        base.join("organiser").join("settings.toml")
    }

    /// Values that make no sense put right: colours that are not `#rrggbb` are dropped (and the
    /// default palette used if none are left), and the trash is kept at least a day.
    pub fn normalized(mut self) -> Self {
        self.tags.palette.retain(|c| is_hex(c));
        for c in &mut self.tags.palette {
            c.make_ascii_lowercase();
        }
        if self.tags.palette.is_empty() {
            self.tags.palette = DEFAULT_PALETTE.map(String::from).to_vec();
        }
        self.notes.trash_days = self.notes.trash_days.clamp(1, 3650);
        self.sticky.palette.retain(|c| is_hex(c));
        for c in &mut self.sticky.palette {
            c.make_ascii_lowercase();
        }
        if self.sticky.palette.is_empty() {
            self.sticky.palette = DEFAULT_STICKY_PALETTE.map(String::from).to_vec();
        }
        if is_hex(&self.sticky.default_color) {
            self.sticky.default_color.make_ascii_lowercase();
        } else {
            self.sticky.default_color = self.sticky.palette[0].clone();
        }
        self
    }

    /// The palette as `&str`s, for the engine.
    pub fn palette(&self) -> Vec<&str> {
        self.tags.palette.iter().map(String::as_str).collect()
    }

    pub fn load() -> Loaded {
        Self::load_from(&Self::path())
    }

    pub fn load_from(path: &Path) -> Loaded {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Loaded {
                    settings: Self::default(),
                    problem: None,
                };
            }
            Err(e) => {
                return Loaded {
                    settings: Self::default(),
                    problem: Some(format!("cannot read {}: {e}", path.display())),
                };
            }
        };
        match toml::from_str::<Settings>(&text) {
            Ok(s) => Loaded {
                settings: s.normalized(),
                problem: None,
            },
            Err(e) => Loaded {
                settings: Self::default(),
                problem: Some(format!("{} is not valid settings: {e}", path.display())),
            },
        }
    }

    pub fn save(&self) -> Result<(), String> {
        self.save_to(&Self::path())
    }

    /// Write the file, whole or not at all: a half-written settings file must never be what the
    /// next start finds.
    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let text = toml::to_string_pretty(&self.clone().normalized())
            .map_err(|e| format!("cannot write settings: {e}"))?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        }
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| format!("cannot replace {}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("organiser-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn no_file_means_the_defaults_and_no_problem() {
        let l = Settings::load_from(&dir("none").join("settings.toml"));
        assert_eq!(l.settings, Settings::default());
        assert_eq!(l.problem, None);
        assert_eq!(l.settings.notes.trash_days, 30);
        assert_eq!(l.settings.tags.palette.len(), 10);
    }

    #[test]
    fn settings_survive_a_save_and_load() {
        let path = dir("round").join("deep").join("settings.toml");
        let mut s = Settings::default();
        s.appearance.theme = ThemeChoice::Dark;
        s.todo.start_view = StartView::List;
        s.todo.show_closed = true;
        s.notes.trash_days = 7;
        s.tags.palette = vec!["#112233".into(), "#aabbcc".into()];
        s.save_to(&path).unwrap();
        assert!(
            !path.with_extension("toml.tmp").exists(),
            "no half-written file is left"
        );
        assert_eq!(Settings::load_from(&path).settings, s);
    }

    #[test]
    fn a_partial_file_keeps_the_defaults_for_what_it_leaves_out() {
        let path = dir("partial").join("settings.toml");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "[appearance]\ntheme = \"light\"\n\n[unknown]\nkey = 1\n",
        )
        .unwrap();
        let l = Settings::load_from(&path);
        assert_eq!(l.problem, None);
        assert_eq!(l.settings.appearance.theme, ThemeChoice::Light);
        assert_eq!(l.settings.notes.trash_days, 30);
    }

    #[test]
    fn nonsense_values_are_put_right() {
        let path = dir("nonsense").join("settings.toml");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "[tags]\npalette = [\"red\", \"#ABCDEF\", \"#12\"]\n[notes]\ntrash_days = 0\n",
        )
        .unwrap();
        let s = Settings::load_from(&path).settings;
        assert_eq!(
            s.tags.palette,
            ["#abcdef"],
            "only real colours stay, lowercased"
        );
        assert_eq!(s.notes.trash_days, 1, "at least a day");
        std::fs::write(&path, "[tags]\npalette = [\"nope\"]\n").unwrap();
        assert_eq!(
            Settings::load_from(&path).settings.tags.palette.len(),
            10,
            "none left: the defaults"
        );
    }

    #[test]
    fn an_unreadable_file_gives_the_defaults_and_says_why_and_is_not_replaced() {
        let path = dir("bad").join("settings.toml");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "this is = = not toml [").unwrap();
        let l = Settings::load_from(&path);
        assert_eq!(l.settings, Settings::default());
        assert!(
            l.problem
                .as_deref()
                .is_some_and(|p| p.contains("not valid settings"))
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "this is = = not toml ["
        );
    }

    #[test]
    fn the_default_palette_is_valid() {
        assert!(DEFAULT_PALETTE.iter().all(|c| is_hex(c)));
        assert!(!is_hex("#12345g") && !is_hex("123456") && !is_hex("#1234567"));
    }
}
