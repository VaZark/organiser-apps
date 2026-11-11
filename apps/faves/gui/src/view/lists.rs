//! The lists: pick the favourites or a list, or start a new one. A dialog on a wide window, a
//! full-screen sheet on a narrow one (it replaces what used to be a sidebar).

use iced::Length;
use iced::widget::{column, row};
use organiser_ui::Element;
use organiser_ui::components::row::selectable;
use organiser_ui::iad::prelude::*;
use organiser_ui::primitives::text::ellipsize;
use organiser_ui::widgets::filter_sheet::filter_sheet;

use crate::app::{App, Message, View};

fn item<'a>(
    icon: Icon,
    title: String,
    detail: String,
    selected: bool,
    on_press: Message,
) -> Element<'a, Message> {
    selectable(
        row![
            icon.view(18.0),
            text(title).width(Length::Fill),
            caption(detail)
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center)
        .padding([10, 14]),
        selected,
        on_press,
    )
}

impl App {
    pub(super) fn lists_panel(&self) -> Element<'_, Message> {
        let favorites: usize = self.lists.iter().map(|s| s.favorites).sum();
        let total: usize = self.lists.iter().map(|s| s.entries).sum();
        let mut rows: Vec<Element<'_, Message>> = vec![
            item(
                Icon::CardGrid,
                "All".into(),
                format!("{total}"),
                self.view == View::All,
                Message::Show(View::All),
            ),
            item(
                Icon::Heart,
                "Favorites".into(),
                format!("{favorites}"),
                self.view == View::Favorites,
                Message::Show(View::Favorites),
            ),
        ];
        for s in &self.lists {
            let mut extras = vec![format!("{} entries", s.entries)];
            if s.list.rated {
                extras.push("rated".into());
            }
            rows.push(item(
                Icon::Folder,
                ellipsize(&s.list.title, 40),
                extras.join(" · "),
                self.view == View::List(s.list.id),
                Message::Show(View::List(s.list.id)),
            ));
        }
        let body = column![
            boxed_list(rows),
            flat_button(row![Icon::ListAdd.view(16.0), text("New list")].spacing(8))
                .on_press(Message::NewList),
        ]
        .spacing(16);
        filter_sheet(
            "Lists",
            Some("Choose what to look at".into()),
            body.into(),
            Message::CloseLists,
            None,
            None,
        )
    }
}
