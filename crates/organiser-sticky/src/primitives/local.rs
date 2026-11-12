//! What Sticky Notes keeps for itself about an item of a linked app, apart from the app's own data:
//! whether it is shown on the desktop, and its colour. The app decides *that* an item is a sticky
//! (its `sticky` flag, which Sticky only reads); this is how Sticky shows it.
//!
//! Only what differs from the defaults is kept: an item is shown on the desktop unless it was
//! hidden, and has no colour of its own unless one was picked.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalItem {
    pub app_id: String,
    pub id: i64,
    /// Shown as a window on the desktop (true, the default), or only in Sticky Notes' own window.
    #[serde(default = "yes")]
    pub show_on_desktop: bool,
    /// `#rrggbb`, over any colour the app gave the item.
    #[serde(default)]
    pub color: Option<String>,
}

impl LocalItem {
    fn is_default(&self) -> bool {
        self.show_on_desktop && self.color.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Local {
    #[serde(rename = "item")]
    pub items: Vec<LocalItem>,
}

impl Local {
    pub fn get(&self, app_id: &str, id: i64) -> Option<&LocalItem> {
        self.items.iter().find(|i| i.app_id == app_id && i.id == id)
    }

    /// Whether to show the item on the desktop: yes, unless it was hidden.
    pub fn show_on_desktop(&self, app_id: &str, id: i64) -> bool {
        self.get(app_id, id).is_none_or(|i| i.show_on_desktop)
    }

    pub fn color(&self, app_id: &str, id: i64) -> Option<&str> {
        self.get(app_id, id).and_then(|i| i.color.as_deref())
    }

    fn entry(&mut self, app_id: &str, id: i64) -> &mut LocalItem {
        if let Some(i) = self
            .items
            .iter()
            .position(|i| i.app_id == app_id && i.id == id)
        {
            return &mut self.items[i];
        }
        self.items.push(LocalItem {
            app_id: app_id.into(),
            id,
            show_on_desktop: true,
            color: None,
        });
        self.items.last_mut().expect("just pushed")
    }

    fn tidy(&mut self) {
        self.items.retain(|i| !i.is_default());
    }

    pub fn set_show_on_desktop(&mut self, app_id: &str, id: i64, on: bool) {
        self.entry(app_id, id).show_on_desktop = on;
        self.tidy();
    }

    pub fn set_color(&mut self, app_id: &str, id: i64, color: Option<String>) {
        self.entry(app_id, id).color = color;
        self.tidy();
    }

    /// Keep an app's entries only for the items still flagged in it (`ids`): an item that stopped
    /// being a sticky starts afresh, with the defaults, if it becomes one again.
    pub fn retain_flagged(&mut self, app_id: &str, ids: &HashSet<i64>) -> bool {
        let n = self.items.len();
        self.items
            .retain(|i| i.app_id != app_id || ids.contains(&i.id));
        self.items.len() != n
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    pub fn render(&self) -> Result<String, String> {
        toml::to_string_pretty(self).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_item_is_shown_on_the_desktop_unless_hidden() {
        let mut l = Local::default();
        assert!(l.show_on_desktop("a", 1));
        l.set_show_on_desktop("a", 1, false);
        assert!(!l.show_on_desktop("a", 1));
        assert!(l.show_on_desktop("a", 2), "other items are untouched");
        l.set_show_on_desktop("a", 1, true);
        assert!(l.items.is_empty(), "back to the default keeps nothing");
    }

    #[test]
    fn colours_and_visibility_are_kept_together_and_tidied() {
        let mut l = Local::default();
        l.set_color("a", 1, Some("#ff0000".into()));
        l.set_show_on_desktop("a", 1, false);
        assert_eq!(l.items.len(), 1);
        assert_eq!(l.color("a", 1), Some("#ff0000"));
        l.set_color("a", 1, None);
        assert_eq!(l.items.len(), 1, "still hidden");
        l.set_show_on_desktop("a", 1, true);
        assert!(l.items.is_empty());
    }

    #[test]
    fn an_item_that_is_no_longer_flagged_is_forgotten() {
        let mut l = Local::default();
        l.set_show_on_desktop("a", 1, false);
        l.set_show_on_desktop("a", 2, false);
        l.set_show_on_desktop("b", 9, false);
        assert!(l.retain_flagged("a", &HashSet::from([2])));
        assert!(l.show_on_desktop("a", 1), "afresh: shown by default");
        assert!(!l.show_on_desktop("a", 2));
        assert!(!l.show_on_desktop("b", 9), "another app is not touched");
    }

    #[test]
    fn it_round_trips_and_missing_keys_take_the_defaults() {
        let mut l = Local::default();
        l.set_show_on_desktop("a", 1, false);
        l.set_color("b", 2, Some("#123456".into()));
        assert_eq!(Local::parse(&l.render().unwrap()).unwrap(), l);
        let p = Local::parse("[[item]]\napp_id = \"a\"\nid = 3\n").unwrap();
        assert!(p.show_on_desktop("a", 3));
        assert_eq!(Local::parse("").unwrap(), Local::default());
    }
}
