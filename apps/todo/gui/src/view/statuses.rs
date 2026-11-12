//! The statuses page of Preferences: name them, colour them, say which mean finished, put them in
//! order. The order is the kanban columns, left to right.

use iced::Length;
use iced::widget::{column, row};
use organiser_ui::Element;
use organiser_ui::components::dot::{color_from_hex, dot};
use organiser_ui::components::toggle::IconToggle;
use organiser_ui::iad::prelude::*;
use organiser_ui::iad::primitives::color_picker::Hsva;
use todo_schema::status::Status;

use crate::app::{App, Message};

/// The colours offered in the picker beside the free choice.
pub(super) const SWATCHES: [&str; 8] = [
    "#8e99a8", "#6aa5e8", "#7fc97f", "#f2d45c", "#f0a35e", "#e0787a", "#a98be0", "#e88fc0",
];

impl App {
    /// The statuses page of Preferences: the list, and a way to add one.
    pub(super) fn statuses_page(&self) -> Element<'_, Message> {
        let n = self.statuses.all().len();
        let mut group = preferences_group().title("Statuses").description(
            "Each status is a column on the board, in this order. A finished status hides its tasks until you ask to see them.",
        );
        for (i, s) in self.statuses.all().iter().enumerate() {
            group = group.push(self.status_row(s, i, n));
        }
        column![
            group,
            flat_button(row![Icon::ListAdd.view(16.0), text("Add a status")].spacing(6))
                .on_press(Message::StatusNew),
        ]
        .spacing(12)
        .into()
    }

    fn status_row<'a>(&'a self, s: &'a Status, at: usize, count: usize) -> Element<'a, Message> {
        let id = s.id;
        let hex = s.color.as_deref().unwrap_or("#9aa0a6");
        let current = color_from_hex(hex)
            .map(|c| {
                Hsva::from_rgb8(
                    (c.r * 255.0) as u8,
                    (c.g * 255.0) as u8,
                    (c.b * 255.0) as u8,
                )
            })
            .unwrap_or_else(|| Hsva::from_rgb8(0x9a, 0xa0, 0xa6));
        let typed = self.status_text.get(&id).map_or(hex, String::as_str);
        let name = self
            .status_names
            .get(&id)
            .map_or(s.name.as_str(), String::as_str);

        let color = ColorInput::new(
            current,
            typed,
            move |t| Message::StatusColorText(id, t),
            move |h| Message::StatusColor(id, h),
        )
        .swatches(SWATCHES)
        .trigger(dot(
            color_from_hex(hex).unwrap_or(iced::Color::from_rgb8(0x9a, 0xa0, 0xa6)),
            18.0,
        ));
        let mut up = icon_button(Icon::GoUp);
        if at > 0 {
            up = up.on_press(Message::StatusMove(id, -1));
        }
        let mut down = icon_button(Icon::GoDown);
        if at + 1 < count {
            down = down.on_press(Message::StatusMove(id, 1));
        }
        row![
            color,
            entry("Name", name)
                .on_input(move |n| Message::StatusName(id, n))
                .width(Length::Fill),
            IconToggle::new(Icon::CheckCircle, s.closed)
                .tooltip("Finished: its tasks are hidden until you ask to see them")
                .on_toggle(move |c| Message::StatusClosed(id, c)),
            up,
            down,
            tooltip(
                icon_button(Icon::Delete).on_press(Message::StatusDelete(id)),
                "Delete (its tasks move to another status)",
                TooltipPosition::Bottom
            ),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .padding([6, 10])
        .width(Length::Fill)
        .into()
    }
}
