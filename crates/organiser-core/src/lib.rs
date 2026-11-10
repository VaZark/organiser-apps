//! App-agnostic basics shared by every organiser app: who the app is and where its files live.

pub mod paths;

/// Static identity of one app. Shared by its engine schema, CLI, GUI and packaging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppMeta {
    /// Reverse-DNS application id; also the `.desktop` basename, e.g. `io.github.vazark.Bookmarks`.
    pub id: &'static str,
    /// Short lowercase name used for directories and the CLI binary, e.g. `bookmarks`.
    pub slug: &'static str,
    /// Display name, e.g. `Bookmarks`.
    pub name: &'static str,
    /// One-line description for the `.desktop` file and `--help`.
    pub comment: &'static str,
    /// Freedesktop menu categories, e.g. `["Office", "Network"]`.
    pub categories: &'static [&'static str],
}
