//! The engine: the registry, the linked databases and the stickies found in them.

use std::collections::{HashMap, HashSet};

use organiser_engine::{Db, Item, ItemPatch};
use organiser_sticky::{Link, Links, Local, STICKY, flag, store};
use serde_json::Value;

use crate::primitives::item::{Changes, Key, StickyItem};

#[derive(Default)]
pub struct Engine {
    links: Links,
    dbs: HashMap<String, Db>,
    items: Vec<StickyItem>,
    local: Local,
}

impl Engine {
    /// An engine that has looked at nothing yet; call [`Engine::refresh`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Every app that offers stickies, switched on or not.
    pub fn links(&self) -> &Links {
        &self.links
    }

    /// The stickies of the linked apps, most recently changed first within each app.
    pub fn items(&self) -> &[StickyItem] {
        &self.items
    }

    pub fn item(&self, key: &Key) -> Option<&StickyItem> {
        self.items.iter().find(|i| &i.key == key)
    }

    /// Read the registry and the flagged items again. Databases of apps that are off, or gone,
    /// are closed; a database that does not exist yet is skipped (opening it would create an
    /// empty one in the app's place). Says what changed since the last time.
    pub fn refresh(&mut self) -> Result<Changes, String> {
        self.links = store::load()?;
        self.local = store::load_local()?;
        let on: HashSet<&str> = self.links.enabled().map(|l| l.app_id.as_str()).collect();
        self.dbs.retain(|id, _| on.contains(id.as_str()));

        let mut found = Vec::new();
        // The flagged ids of each app that could be read, to forget what Sticky kept about items
        // that stopped being stickies.
        let mut flagged: HashMap<String, HashSet<i64>> = HashMap::new();
        for link in self.links.enabled() {
            if !self.dbs.contains_key(&link.app_id) {
                match Db::open_loose(&link.db_path) {
                    Ok(db) => {
                        self.dbs.insert(link.app_id.clone(), db);
                    }
                    Err(_) => continue,
                }
            }
            let Some(db) = self.dbs.get(&link.app_id) else {
                continue;
            };
            let mut ids = HashSet::new();
            let mut complete = true;
            for k in &link.kinds {
                match db.flagged_loose(&k.kind, STICKY) {
                    Ok(items) => {
                        ids.extend(items.iter().map(|i| i.id));
                        found.extend(
                            items
                                .iter()
                                .map(|i| to_item(link, &k.kind, &k.body, i, &self.local)),
                        );
                    }
                    Err(_) => complete = false,
                }
            }
            if complete {
                flagged.insert(link.app_id.clone(), ids);
            }
        }
        let mut pruned = false;
        for (app, ids) in &flagged {
            pruned |= self.local.retain_flagged(app, ids);
        }
        if pruned {
            store::save_local(&self.local)?;
        }

        // Seconds are too coarse to tell two edits apart: compare what is shown.
        let old: HashMap<&Key, &StickyItem> = self.items.iter().map(|i| (&i.key, i)).collect();
        let now: HashSet<&Key> = found.iter().map(|i| &i.key).collect();
        let mut changes = Changes::default();
        for i in &found {
            match old.get(&i.key) {
                None => changes.added.push(i.key.clone()),
                Some(was) if *was != i => changes.updated.push(i.key.clone()),
                Some(_) => {}
            }
        }
        changes.removed = old
            .keys()
            .filter(|k| !now.contains(**k))
            .map(|k| (*k).clone())
            .collect();
        changes.removed.sort();
        self.items = found;
        Ok(changes)
    }

    /// Write an edit of the item's own data (title, text) to the app's database. The flag that
    /// makes it a sticky is never written: it is the app's.
    fn edit(&mut self, key: &Key, patch: ItemPatch) -> Result<(), String> {
        let item = self.item(key).ok_or("that sticky is gone")?;
        let searchable = item.body_field.clone();
        let db = self
            .dbs
            .get_mut(&key.app_id)
            .ok_or("that app is not linked")?;
        let done = db
            .update_loose(key.id, patch, &[&searchable])
            .map_err(|e| e.to_string())?;
        if let Some(i) = self.items.iter_mut().find(|i| &i.key == key) {
            i.title = done.title.clone();
            i.body = done
                .fields
                .get(&i.body_field)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into();
            i.updated = done.updated;
        }
        Ok(())
    }

    pub fn set_title(&mut self, key: &Key, title: &str) -> Result<(), String> {
        self.edit(key, ItemPatch::default().title(title))
    }

    pub fn set_body(&mut self, key: &Key, body: &str) -> Result<(), String> {
        let field = self
            .item(key)
            .ok_or("that sticky is gone")?
            .body_field
            .clone();
        self.edit(key, ItemPatch::default().field(field.as_str(), body))
    }

    /// Paint a sticky (`#rrggbb`), or `None` to go back to the app's colour or the default paper.
    /// Kept by Sticky Notes, not written to the app.
    pub fn set_color(&mut self, key: &Key, color: Option<&str>) -> Result<(), String> {
        self.item(key).ok_or("that sticky is gone")?;
        let mut local = store::load_local()?;
        local.set_color(&key.app_id, key.id, color.map(String::from));
        store::save_local(&local)?;
        self.local = local;
        if let Some(i) = self.items.iter_mut().find(|i| &i.key == key) {
            i.color = color.map(String::from).or_else(|| i.app_color.clone());
        }
        Ok(())
    }

    /// Show a sticky as a window on the desktop (the default), or only in Sticky Notes' main
    /// window. Kept by Sticky Notes: the item in its app is not touched, and stays a sticky.
    pub fn set_show_on_desktop(&mut self, key: &Key, on: bool) -> Result<(), String> {
        self.item(key).ok_or("that sticky is gone")?;
        let mut local = store::load_local()?;
        local.set_show_on_desktop(&key.app_id, key.id, on);
        store::save_local(&local)?;
        self.local = local;
        if let Some(i) = self.items.iter_mut().find(|i| &i.key == key) {
            i.show_on_desktop = on;
        }
        Ok(())
    }

    /// Switch an app on or off, then look again.
    pub fn set_linked(&mut self, app_id: &str, on: bool) -> Result<Changes, String> {
        store::set_enabled_by_id(app_id, on)?;
        self.refresh()
    }
}

fn to_item(link: &Link, kind: &str, body: &str, i: &Item, local: &Local) -> StickyItem {
    let app_color = flag::color(i).map(String::from);
    StickyItem {
        key: Key::new(&link.app_id, i.id),
        app_name: link.title().into(),
        kind: kind.into(),
        body_field: body.into(),
        title: i.title.clone(),
        body: i
            .fields
            .get(body)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into(),
        color: local
            .color(&link.app_id, i.id)
            .map(String::from)
            .or_else(|| app_color.clone()),
        app_color,
        show_on_desktop: local.show_on_desktop(&link.app_id, i.id),
        updated: i.updated,
    }
}
