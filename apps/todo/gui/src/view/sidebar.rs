//! The board's sidebar: the epics, then the tags with how many tasks each has. (Whether closed
//! tasks show is the header's toggle, as in the list.) Only the board has one; the list has none.

use organiser_ui::Element;
use organiser_ui::iad::prelude::*;
use organiser_ui::widgets::tag_tree;

use crate::app::{App, Message};

impl App {
    pub(super) fn sidebar(&self) -> Element<'_, Message> {
        let mut items: Vec<SidebarItem<Message>> = Vec::new();

        for s in self.epics.iter().filter(|s| s.epic.status.is_open()) {
            let title = format!("{}  ({}/{})", s.epic.title, s.total - s.open, s.total);
            let item = SidebarItem::new(title, Message::ToggleEpic(s.epic.id))
                .icon(Icon::Document)
                .selected(self.epic_filter == Some(s.epic.id));
            items.push(item);
        }
        items.push(SidebarItem::new("New epic", Message::NewEpic).icon(Icon::ListAdd));

        let mut first = true;
        for (depth, t) in tag_tree::ordered(&self.tags) {
            let name = t.name.rsplit(':').next().unwrap_or(&t.name);
            let mut item = SidebarItem::new(
                format!("{}{name}  ({})", "   ".repeat(depth), t.count("item")),
                Message::ToggleTag(t.name.clone()),
            )
            .icon(Icon::Tag)
            .selected(self.tag_filters.contains(&t.name));
            if first {
                item = item.divider_above();
                first = false;
            }
            items.push(item);
        }
        SidebarList::new(items).style(SidebarStyle::List).into()
    }
}
