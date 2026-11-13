//! The sidebar, in the board and the list: All, the tasks that are current today, then the epics
//! to pick from. Tags and statuses are not here: tags are
//! found through the filter, in their own colours, and both have a button in the header. Only the
//! board has a sidebar; the list has none.

use organiser_ui::Element;
use organiser_ui::components::icons;
use organiser_ui::iad::prelude::*;
use todo_schema::views::Scope;

use crate::app::{App, Message};

impl App {
    pub(super) fn sidebar(&self) -> Element<'_, Message> {
        // "All" and the epics are one choice, a selection: picking an epic shows its tasks, picking
        // All shows everything. (The filter can still pick several.)
        let everything = self.scope == Scope::All && self.epic_filter.is_none() && !self.no_epic;
        let mut items: Vec<SidebarItem<Message>> = vec![
            SidebarItem::new("All", Message::SetEpic(None))
                .icon(Icon::CardGrid)
                .selected(everything),
            SidebarItem::new("In progress", Message::Current(true))
                .icon(icons::calendar())
                .selected(self.scope == Scope::Current),
            SidebarItem::new("Archived", Message::Archived)
                .icon(Icon::Archive)
                .selected(self.scope == Scope::Archived),
        ];
        // The tasks that are in no epic come first among the epics.
        let (open, total) = self.unassigned;
        items.push(
            SidebarItem::new(
                format!("Unassigned  ({}/{})", total - open, total),
                Message::Unassigned,
            )
            .icon(Icon::Folder)
            .icon_color(organiser_ui::components::color_pick::UNSET)
            .selected(self.scope == Scope::All && self.no_epic)
            .divider_above()
            // A task dropped on it leaves its epic.
            .drop_target("Take out of its epic", |_| Message::DropInEpic(None)),
        );
        let mut first = false;
        for s in self
            .epics
            .iter()
        {
            let title = format!("{}  ({}/{})", s.epic.title, s.total - s.open, s.total);
            let mut item = SidebarItem::new(title, Message::SetEpic(Some(s.epic.id)))
                .icon(icons::named(s.epic.icon.as_deref()))
                .selected(self.scope == Scope::All && self.epic_filter == Some(s.epic.id));
            // The folder wears the epic's colour.
            if let Some(c) = self.epic_color(s.epic.id) {
                item = item.icon_color(c);
            }

            // Press and hold an epic to move it: the others take it before or after themselves.
            let id = s.epic.id;
            item = item.reorder(
                Message::EpicDragStart(id),
                "Move here",
                move |place| Message::DropEpic(id, place),
            );

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

        // All and Current tasks stay in view; the epics scroll beneath them.
        SidebarList::new(items)
            .style(SidebarStyle::List)
            .fixed_top(3)
            .dragging(self.dragging_epic.is_some())
            .into()
    }
}
