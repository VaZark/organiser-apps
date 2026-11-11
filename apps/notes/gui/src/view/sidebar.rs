//! The sidebar: where notes live (Notes, Archive, Trash). Tags are not here: they are found through
//! the filter, in their own colours.

use notes_schema::views::Scope;
use organiser_ui::Element;
use organiser_ui::iad::prelude::*;

use crate::app::{App, Message};

impl App {
    pub(super) fn sidebar(&self) -> Element<'_, Message> {
        let place = |label: &str, icon: Icon, scope: Scope| {
            SidebarItem::new(label.to_string(), Message::SetScope(scope))
                .icon(icon)
                .selected(self.scope == scope)
        };
        let items = vec![
            place("Notes", Icon::Note, Scope::Notes),
            place("Archive", Icon::Archive, Scope::Archive),
            place("Trash", Icon::Delete, Scope::Trash),
        ];
        SidebarList::new(items).style(SidebarStyle::List).into()
    }
}
