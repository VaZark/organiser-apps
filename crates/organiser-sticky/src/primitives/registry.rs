//! The apps that have published themselves to Sticky Notes, and which of them are switched on.

use organiser_core::sticky::StickySpec;
use serde::{Deserialize, Serialize};

/// One kind of item an app lets be a sticky, as stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KindSpec {
    pub kind: String,
    /// The field holding the item's text.
    pub body: String,
}

/// An app that offers stickies. `enabled` is the "Link to sticky notes" switch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Link {
    /// The app's application id, e.g. `io.github.vazark.Notes`.
    pub app_id: String,
    /// Display name, e.g. `Notes`.
    pub name: String,
    /// The app's slug, e.g. `notes`.
    pub slug: String,
    /// The app's database file.
    pub db_path: String,
    pub kinds: Vec<KindSpec>,
    pub enabled: bool,
}

impl Default for Link {
    fn default() -> Self {
        Self {
            app_id: String::new(),
            name: String::new(),
            slug: String::new(),
            db_path: String::new(),
            kinds: Vec::new(),
            // A link written before apps published themselves was a link.
            enabled: true,
        }
    }
}

impl Link {
    /// What to call the app: its name, else its slug.
    pub fn title(&self) -> &str {
        if self.name.is_empty() {
            &self.slug
        } else {
            &self.name
        }
    }

    /// An app's offer as a link, switched off.
    pub fn from_spec(spec: &StickySpec, db_path: String) -> Self {
        Self {
            app_id: spec.meta.id.into(),
            name: spec.meta.name.into(),
            slug: spec.meta.slug.into(),
            db_path,
            kinds: spec
                .kinds
                .iter()
                .map(|k| KindSpec {
                    kind: k.kind.into(),
                    body: k.body.into(),
                })
                .collect(),
            enabled: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Links {
    #[serde(rename = "link")]
    pub links: Vec<Link>,
}

impl Links {
    pub fn get(&self, app_id: &str) -> Option<&Link> {
        self.links.iter().find(|l| l.app_id == app_id)
    }

    /// Whether the app is published and switched on.
    pub fn is_linked(&self, app_id: &str) -> bool {
        self.get(app_id).is_some_and(|l| l.enabled)
    }

    /// The apps whose stickies are shown.
    pub fn enabled(&self) -> impl Iterator<Item = &Link> {
        self.links.iter().filter(|l| l.enabled)
    }

    /// Publish an app: add it (switched off) or update what it says about itself (its database,
    /// its kinds), keeping the switch as it is. Whether anything changed.
    pub fn publish(&mut self, link: Link) -> bool {
        match self.links.iter_mut().find(|l| l.app_id == link.app_id) {
            Some(old) => {
                let new = Link {
                    enabled: old.enabled,
                    ..link
                };
                let changed = *old != new;
                *old = new;
                changed
            }
            None => {
                self.links.push(link);
                true
            }
        }
    }

    /// Remove an app; whether it was there.
    pub fn remove(&mut self, app_id: &str) -> bool {
        let n = self.links.len();
        self.links.retain(|l| l.app_id != app_id);
        self.links.len() != n
    }

    /// Switch an app on or off; whether it is published at all.
    pub fn set_enabled(&mut self, app_id: &str, on: bool) -> bool {
        match self.links.iter_mut().find(|l| l.app_id == app_id) {
            Some(l) => {
                l.enabled = on;
                true
            }
            None => false,
        }
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

    fn l(id: &str, db: &str) -> Link {
        Link {
            app_id: id.into(),
            slug: "x".into(),
            db_path: db.into(),
            enabled: false,
            ..Link::default()
        }
    }

    #[test]
    fn publishing_again_updates_the_app_and_keeps_the_switch() {
        let mut s = Links::default();
        assert!(s.publish(l("a", "/1")));
        assert!(!s.is_linked("a"), "a new app starts switched off");
        s.set_enabled("a", true);
        assert!(s.publish(l("a", "/2")));
        assert_eq!(s.links.len(), 1);
        assert_eq!(s.get("a").unwrap().db_path, "/2");
        assert!(s.is_linked("a"), "still on");
        assert!(!s.publish(l("a", "/2")), "nothing changed");
    }

    #[test]
    fn only_published_apps_can_be_switched() {
        let mut s = Links::default();
        assert!(!s.set_enabled("a", true));
        s.publish(l("a", "/1"));
        assert!(s.set_enabled("a", true));
        assert_eq!(s.enabled().count(), 1);
        s.set_enabled("a", false);
        assert_eq!(s.enabled().count(), 0);
    }

    #[test]
    fn it_round_trips_through_toml_and_reads_the_old_format() {
        let mut s = Links::default();
        s.publish(l("a", "/1"));
        s.publish(l("b", "/2"));
        assert_eq!(Links::parse(&s.render().unwrap()).unwrap(), s);
        assert_eq!(Links::parse("").unwrap(), Links::default());
        // Before apps published themselves a link had only these keys, and was on.
        let old =
            Links::parse("[[link]]\napp_id = \"a\"\nslug = \"notes\"\ndb_path = \"/x\"\n").unwrap();
        assert!(old.is_linked("a"));
        assert_eq!(old.get("a").unwrap().title(), "notes");
    }
}
