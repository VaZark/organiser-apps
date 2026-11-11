//! The sidebar, in the board and the list: All, the tasks that are current today, then the epics
//! to pick from. Tags and statuses are not here: tags are
//! found through the filter, in their own colours, and both have a button in the header. Only the
//! board has a sidebar; the list has none.

use organiser_ui::Element;
use organiser_ui::iad::prelude::*;
use todo_schema::views::Scope;

use crate::app::{App, Message};

impl App {
    pub(super) fn sidebar(&self) -> Element<'_, Message> {
        // "All" and the epics are one choice, a selection: picking an epic shows its tasks, picking
        // All shows everything. (The filter can still pick several.)
        let everything = self.scope == Scope::All && self.epic_filter.is_none();
        let mut items: Vec<SidebarItem<Message>> = vec![
            SidebarItem::new("All", Message::SetEpic(None))
                .icon(Icon::CardGrid)
                .selected(everything),
            SidebarItem::new("Current tasks", Message::Current(true))
                .icon(Icon::Calendar)
                .selected(self.scope == Scope::Current),
        ];
        let mut first = true;
        for s in self
            .epics
            .iter()
            .filter(|s| self.statuses.is_open(s.epic.status))
        {
            let title = format!("{}  ({}/{})", s.epic.title, s.total - s.open, s.total);
            let mut item = SidebarItem::new(title, Message::SetEpic(Some(s.epic.id)))
                .icon(Icon::Document)
                .selected(self.scope == Scope::All && self.epic_filter == Some(s.epic.id));
            if first {
                item = item.divider_above();
                first = false;
            }
            items.push(item);
        }
        let mut new_epic = SidebarItem::new("New epic", Message::NewEpic).icon(Icon::ListAdd);
        if first {
            new_epic = new_epic.divider_above();
        }
        items.push(new_epic);

        SidebarList::new(items).style(SidebarStyle::List).into()
    }
}
