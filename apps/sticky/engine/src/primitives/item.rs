//! A sticky as the frontends see it.

/// Which item of which app: the app's id and the item's id in its database.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key {
    pub app_id: String,
    pub id: i64,
}

impl Key {
    pub fn new(app_id: &str, id: i64) -> Self {
        Self {
            app_id: app_id.into(),
            id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StickyItem {
    pub key: Key,
    /// The app's display name.
    pub app_name: String,
    pub kind: String,
    /// The field of the item holding its text.
    pub body_field: String,
    pub title: String,
    pub body: String,
    /// `#rrggbb`: Sticky's own colour for it if one was picked, else the app's; none means the
    /// default paper.
    pub color: Option<String>,
    /// The colour the app gave the item, which Sticky never changes.
    pub app_color: Option<String>,
    /// Shown as a window on the desktop (Sticky's own choice; on unless hidden), or only in
    /// Sticky Notes' main window.
    pub show_on_desktop: bool,
    /// The item's `updated`, to tell whether it changed.
    pub updated: i64,
}

/// What a refresh found different.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    pub added: Vec<Key>,
    pub removed: Vec<Key>,
    /// Still stickies, with a different title, text or colour.
    pub updated: Vec<Key>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.updated.is_empty()
    }
}
