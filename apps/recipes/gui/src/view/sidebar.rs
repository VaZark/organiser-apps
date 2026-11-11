//! The sidebar of a wide window: the three places. Folders and favourites are a switcher at the top
//! of the recipes themselves.

use organiser_ui::Element;
use organiser_ui::iad::prelude::*;

use crate::app::{App, Message, View};

impl App {
    pub(super) fn sidebar(&self) -> Element<'_, Message> {
        let recipes = self.view == View::All;
        let items = vec![
            SidebarItem::new("Recipes", Message::Show(View::All))
                .icon(Icon::CardGrid)
                .selected(recipes),
            SidebarItem::new("Log", Message::Show(View::Calendar))
                .icon(Icon::CheckCircle)
                .selected(self.view == View::Calendar),
            SidebarItem::new("Fridge", Message::Show(View::Fridge))
                .icon(Icon::Archive)
                .selected(self.view == View::Fridge),
        ];
        SidebarList::new(items).style(SidebarStyle::List).into()
    }
}
