//! The sidebar: All, Favorites, the default section ("Unassigned"), Archive and Trash, which stay
//! in view, then the sections, which scroll.
//!
//! Sections nest to any depth, and the sidebar shows one level of them, the way a menu's submenu
//! takes the place of its parent: always the siblings of the section being viewed (the sections at
//! the top when it is at the top), under a back arrow headed with the name of the section they are
//! in. Choosing a section inside the viewed one, from its page, slides that level in; the arrow
//! (or the one in the header bar) slides it away again. Choosing a sibling does not slide.

use notes_schema::views::Scope;
use organiser_ui::Element;
use organiser_ui::components::icons;
use organiser_ui::iad::prelude::*;
use organiser_ui::primitives::text::ellipsize;

use crate::app::{App, Message};

impl App {
    pub(super) fn sidebar(&self) -> Element<'_, Message> {
        let place = |label: String, icon: Icon, scope: Scope| {
            SidebarItem::new(label, Message::SetScope(scope))
                .icon(icon)
                .selected(self.scope == scope)
        };
        let total: usize = self.folders.iter().map(|s| s.notes).sum();
        let favorites: usize = self.folders.iter().map(|s| s.favorites).sum();
        // The places stay in view: All, Favorites, Unassigned (the notes in no section), Archive and
        // Trash; the sections scroll beneath.
        let mut items = vec![
            place(format!("All  ({total})"), Icon::Note, Scope::Notes),
            place(format!("Favorites  ({favorites})"), Icon::Heart, Scope::Favorites),
        ];
        if let Some(home) = self.folders.iter().find(|s| s.folder.is_default) {
            let id = home.folder.id;
            items.push(
                SidebarItem::new(
                    format!("{}  ({})", home.folder.title, home.notes),
                    Message::SetScope(Scope::Folder(id)),
                )
                .icon(icons::named(home.folder.icon.as_deref()))
                .icon_color(self.folder_color(&home.folder))
                .selected(self.scope == Scope::Folder(id))
                // Notes can be dropped on it, to take them out of their section.
                .drop_target("Take out of its section", move |p| Message::DropFolder(id, p)),
            );
        }
        items.push(place("Archive".into(), Icon::Archive, Scope::Archive));
        items.push(place("Trash".into(), Icon::Delete, Scope::Trash));
        let places = items.len();

        // The pages: the top sections, then one for each section that has sections in it.
        let mut levels: Vec<Option<i64>> = vec![None];
        levels.extend(
            self.folders
                .iter()
                .filter(|s| !s.folder.is_default && s.subfolders > 0)
                .map(|s| Some(s.folder.id)),
        );
        let level = self.sidebar_level();
        let current = levels.iter().position(|l| *l == level).unwrap_or(0);
        let depths = levels
            .iter()
            .map(|l| l.map_or(0, |id| self.folder_path(id).len() as u32))
            .collect();
        let pages = levels.iter().map(|l| self.sidebar_page(*l)).collect();
        SidebarList::new(items)
            .style(SidebarStyle::List)
            .fixed_top(places)
            .pages(pages, current, depths)
            .dragging(self.dragging_folder.is_some() || self.dragging.is_some())
            .into()
    }

    /// The section whose sections the sidebar lists: the one the section being viewed is in, so
    /// the sidebar always shows the viewed section's siblings; none at the top. Choosing a section
    /// inside the viewed one (from its page) moves the level down, and the page slides.
    pub(super) fn sidebar_level(&self) -> Option<i64> {
        let Scope::Folder(id) = self.scope else { return None };
        self.folder_summary(id).filter(|s| !s.folder.is_default).and_then(|s| s.folder.parent)
    }

    /// One page of the sidebar: a back arrow with the name of the section whose page it is (none at
    /// the top), the sections in it, and a way to add one.
    fn sidebar_page(&self, level: Option<i64>) -> Vec<SidebarItem<Message>> {
        let mut page = Vec::new();
        if let Some(s) = level.and_then(|id| self.folder_summary(id)) {
            // Up to the section this one is in (or all the notes).
            page.push(
                SidebarItem::new(ellipsize(&s.folder.title, 26), Message::Back).icon(Icon::GoPrevious),
            );
        }
        for s in self.children_of(level).into_iter().filter(|s| !s.folder.is_default) {
            let f = &s.folder;
            let id = f.id;
            let mut item = SidebarItem::new(
                format!("{}  ({})", ellipsize(&f.title, 24), s.total),
                Message::SetScope(Scope::Folder(id)),
            )
            .icon(icons::named(f.icon.as_deref()))
            .icon_color(self.folder_color(f))
            .selected(self.scope == Scope::Folder(id));
            // What can be dropped on it: a note goes in it; a section goes before, after or inside
            // it.
            let drop = move |p| Message::DropFolder(id, p);
            item = if self.dragging.is_some() {
                item.drop_target("Move here", drop)
            } else {
                item.reorder(Message::FolderDragStart(id), "Move here", drop)
                    .nest(format!("Move into {}", ellipsize(&f.title, 24)))
            };
            page.push(item);
        }
        page.push(
            SidebarItem::new("New section", Message::NewSubfolderIn(level)).icon(Icon::ListAdd),
        );
        // A line between the places above and this page's rows.
        let first = page.remove(0).divider_above();
        page.insert(0, first);
        page
    }
}
