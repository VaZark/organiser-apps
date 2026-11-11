//! The engine's front door: one `Db` per app database. Frontends call these methods and nothing
//! below them. Every write validates against the app's schema and keeps search and tags in step.

use std::path::Path;

use rusqlite::Connection;
use weaver::{ChildPolicy, Tag};

use crate::components::{connection, finder, fts, gates, index, items, migrate, refs, tags, tree};
use crate::error::{Error, Result, invalid};
use crate::primitives::clock::now;
use crate::primitives::item::{Item, ItemPatch, NewItem};
use crate::primitives::query::{ParentFilter, Query};
use crate::primitives::schema::Schema;

pub struct Db {
    conn: Connection,
    schema: &'static Schema,
}

impl Db {
    pub fn open(path: impl AsRef<Path>, schema: &'static Schema) -> Result<Self> {
        Self::init(connection::open(path.as_ref())?, schema)
    }

    pub fn open_in_memory(schema: &'static Schema) -> Result<Self> {
        Self::init(connection::open_memory()?, schema)
    }

    fn init(mut conn: Connection, schema: &'static Schema) -> Result<Self> {
        schema.check()?;
        migrate::run(&mut conn)?;
        index::reconcile(&conn, schema)?;
        Ok(Self { conn, schema })
    }

    pub fn schema(&self) -> &'static Schema {
        self.schema
    }

    // ---- items -------------------------------------------------------------------------

    pub fn create(&mut self, new: NewItem) -> Result<Item> {
        let title = new.title.trim().to_string();
        if title.is_empty() {
            return invalid("title must not be empty");
        }
        let mut new = new;
        self.schema.apply_defaults(&new.kind, &mut new.fields);
        self.schema.validate(&new.kind, &new.fields)?;
        let names = tags::parse_all(&new.tags)?;
        let schema = self.schema;
        let tx = self.conn.transaction()?;
        if let Some(p) = new.parent_id
            && !items::exists(&tx, p)?
        {
            return Err(Error::NotFound(p));
        }
        refs::check_fields(&tx, schema, &new.kind, &new.fields)?;
        gates::check(&tx, schema, &new.kind, new.parent_id, &new.fields)?;
        let position = tree::next_position(&tx, new.parent_id)?;
        let id = items::insert(
            &tx,
            &new.kind,
            new.parent_id,
            &title,
            &new.fields,
            position,
            now(),
        )?;
        fts::index(&tx, id, &title, &schema.search_text(&new.kind, &new.fields))?;
        tags::assign(&tx, id, &names)?;
        tx.commit()?;
        self.get(id)
    }

    pub fn get(&self, id: i64) -> Result<Item> {
        items::get(&self.conn, id)
    }

    pub fn update(&mut self, id: i64, patch: ItemPatch) -> Result<Item> {
        let mut item = self.get(id)?;
        if let Some(t) = patch.title {
            item.title = t.trim().to_string();
            if item.title.is_empty() {
                return invalid("title must not be empty");
            }
        }
        for (k, v) in patch.fields {
            if v.is_null() {
                item.fields.remove(&k);
            } else {
                item.fields.insert(k, v);
            }
        }
        if let Some(a) = patch.archived {
            item.archived = a;
        }
        self.schema.validate(&item.kind, &item.fields)?;
        let schema = self.schema;
        let tx = self.conn.transaction()?;
        refs::check_fields(&tx, schema, &item.kind, &item.fields)?;
        gates::check(&tx, schema, &item.kind, item.parent_id, &item.fields)?;
        items::write(&tx, id, &item.title, &item.fields, item.archived, now())?;
        gates::strip(&tx, schema, &item.kind, id, &item.fields)?;
        fts::index(
            &tx,
            id,
            &item.title,
            &schema.search_text(&item.kind, &item.fields),
        )?;
        tx.commit()?;
        self.get(id)
    }

    /// Delete an item and everything below it. Returns how many items went.
    pub fn delete(&mut self, id: i64) -> Result<usize> {
        if !items::exists(&self.conn, id)? {
            return Err(Error::NotFound(id));
        }
        let schema = self.schema;
        let tx = self.conn.transaction()?;
        let ids = tree::subtree(&tx, id)?;
        let gone: Vec<(i64, String)> = ids
            .iter()
            .map(|i| items::get(&tx, *i).map(|it| (it.id, it.kind)))
            .collect::<Result<_>>()?;
        refs::unlink(&tx, schema, &gone)?;
        fts::remove(&tx, &ids)?;
        items::delete(&tx, id)?; // children and tag assignments follow by cascade
        tx.commit()?;
        Ok(ids.len())
    }

    /// Re-parent and/or reorder. `position: None` appends to the new parent.
    pub fn move_item(
        &mut self,
        id: i64,
        parent: Option<i64>,
        position: Option<i64>,
    ) -> Result<Item> {
        if !items::exists(&self.conn, id)? {
            return Err(Error::NotFound(id));
        }
        if let Some(p) = parent
            && !items::exists(&self.conn, p)?
        {
            return Err(Error::NotFound(p));
        }
        // Where it lands must still allow every gated field it carries.
        let item = self.get(id)?;
        let schema = self.schema;
        let tx = self.conn.transaction()?;
        gates::check(&tx, schema, &item.kind, parent, &item.fields)?;
        tree::relocate(&tx, id, parent, position)?;
        tx.commit()?;
        self.get(id)
    }

    pub fn list(&self, q: &Query) -> Result<Vec<Item>> {
        finder::find(&self.conn, self.schema, q)
    }

    pub fn children(&self, parent: Option<i64>) -> Result<Vec<Item>> {
        self.list(&Query {
            parent: parent.map_or(ParentFilter::Root, ParentFilter::Of),
            ..Query::default()
        })
    }

    // ---- tags on items -----------------------------------------------------------------

    pub fn add_tags(&mut self, id: i64, names: &[String]) -> Result<Item> {
        let parsed = tags::parse_all(names)?;
        if !items::exists(&self.conn, id)? {
            return Err(Error::NotFound(id));
        }
        let tx = self.conn.transaction()?;
        tags::assign(&tx, id, &parsed)?;
        tx.commit()?;
        self.get(id)
    }

    pub fn remove_tags(&mut self, id: i64, names: &[String]) -> Result<Item> {
        if !items::exists(&self.conn, id)? {
            return Err(Error::NotFound(id));
        }
        tags::remove(&mut self.conn, id, names)?;
        self.get(id)
    }

    /// Make an item's tags exactly `names`, adding and removing only the difference. Names are
    /// compared ignoring case.
    pub fn set_tags(&mut self, id: i64, names: &[String]) -> Result<Item> {
        let have = self.get(id)?.tags;
        let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
        let remove: Vec<String> = have
            .iter()
            .filter(|h| !names.iter().any(|n| same(h, n)))
            .cloned()
            .collect();
        let add: Vec<String> = names
            .iter()
            .filter(|n| !have.iter().any(|h| same(h, n)))
            .cloned()
            .collect();
        if !remove.is_empty() {
            self.remove_tags(id, &remove)?;
        }
        if !add.is_empty() {
            self.add_tags(id, &add)?;
        }
        self.get(id)
    }

    // ---- the tag vocabulary and tree ---------------------------------------------------

    pub fn tags(&self) -> Result<Vec<Tag>> {
        Ok(tags::TAGS.list(&self.conn)?)
    }

    pub fn create_tag(&mut self, name: &str, parent: Option<&str>) -> Result<Tag> {
        Ok(tags::TAGS.create(&mut self.conn, name, parent)?)
    }

    pub fn set_tag_parent(&mut self, name: &str, parent: Option<&str>) -> Result<Tag> {
        Ok(tags::TAGS.set_parent(&mut self.conn, name, parent)?)
    }

    /// Set (or, with `None`, clear) a tag's colour, `#rrggbb`. Its parent and assignments stay.
    pub fn set_tag_color(&mut self, name: &str, color: Option<&str>) -> Result<Tag> {
        let icon = tags::TAGS.get(&self.conn, name)?.icon;
        Ok(tags::TAGS.set_style(&mut self.conn, name, icon.as_deref(), color)?)
    }

    /// Give every tag that has no colour one from `palette`, picked by its name. Returns how many
    /// were coloured: nothing is done (and nothing written) when all already have one.
    pub fn ensure_tag_colors(&mut self, palette: &[&str]) -> Result<usize> {
        let mut n = 0;
        for t in self.tags()?.into_iter().filter(|t| t.color.is_none()) {
            if let Some(c) = crate::primitives::palette::pick(&t.name, palette) {
                self.set_tag_color(&t.name, Some(c))?;
                n += 1;
            }
        }
        Ok(n)
    }

    pub fn delete_tag(&mut self, name: &str, children: ChildPolicy) -> Result<usize> {
        Ok(tags::TAGS.delete(&mut self.conn, name, children)?)
    }
}
