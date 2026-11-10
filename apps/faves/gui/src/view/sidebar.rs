//! The sidebar of a wide window: the favourites of every list, the lists, and a way to start a new
//! one. A narrow window has none; its title opens the same choices as a dialog.

use organiser_ui::Element;
use organiser_ui::iad::prelude::*;
use organiser_ui::primitives::text::ellipsize;

use crate::app::{App, Message, View};

impl App {
    pub(super) fn sidebar(&self) -> Element<'_, Message> {
        let favorites: usize = self.lists.iter().map(|s| s.favorites).sum();
        let mut items = vec![
            SidebarItem::new(
                format!("Favorites  ({favorites})"),
                Message::Show(View::Favorites),
            )
            .icon(Icon::Heart)
            .selected(self.view == View::Favorites),
        ];
        let mut first = true;
        for s in &self.lists {
            let title = format!("{}  ({})", ellipsize(&s.list.title, 24), s.entries);
            let mut item = SidebarItem::new(title, Message::Show(View::List(s.list.id)))
                .icon(Icon::Folder)
                .selected(self.view == View::List(s.list.id));
            if first {
                item = item.divider_above();
                first = false;
            }
            items.push(item);
        }
        let mut new_list = SidebarItem::new("New list", Message::NewList).icon(Icon::ListAdd);
        if first {
            new_list = new_list.divider_above();
        }
        items.push(new_list);
        SidebarList::new(items).style(SidebarStyle::List).into()
    }
}
