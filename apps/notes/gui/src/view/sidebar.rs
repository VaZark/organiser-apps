//! The sidebar: All, Favorites, Archive and Trash, which stay in view, then the folders (scrolling)
//! and a way to start a new one. Tags are not here: they are found through the filter, in their own colours.

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
        // The places stay in view: All, Favorites, Archive and Trash; the folders scroll beneath.
        let mut items = vec![
            place(format!("All  ({total})"), Icon::Note, Scope::Notes),
            place(format!("Favorites  ({favorites})"), Icon::Heart, Scope::Favorites),
            place("Archive".into(), Icon::Archive, Scope::Archive),
            place("Trash".into(), Icon::Delete, Scope::Trash),
        ];
        let places = items.len();
        let mut first = true;
        for s in &self.folders {
            let title = format!("{}  ({})", ellipsize(&s.folder.title, 24), s.notes);
            let mut item = SidebarItem::new(title, Message::SetScope(Scope::Folder(s.folder.id)))
                .icon(icons::named(s.folder.icon.as_deref()))
                .icon_color(self.folder_color(&s.folder))
                .selected(self.scope == Scope::Folder(s.folder.id));
            if first {
                item = item.divider_above();
                first = false;
            }
            items.push(item);
        }
        let mut new_folder = SidebarItem::new("New folder", Message::NewFolder).icon(Icon::ListAdd);
        if first {
            new_folder = new_folder.divider_above();
        }
        items.push(new_folder);
        SidebarList::new(items)
            .style(SidebarStyle::List)
            .fixed_top(places)
            .into()
    }
}
