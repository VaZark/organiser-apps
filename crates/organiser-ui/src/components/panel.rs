//! The surface of a modal flow: a bounded card on a wide window, the whole window on a narrow one.

use iced::widget::container;
use iced::{Length, Size};

use crate::Element;
use crate::iad;

fn sheet_style(theme: &iad::Theme, window_radius: f32) -> container::Style {
    let mut style = iad::styles::container::window(theme);
    // The sheet overlays the window frame, so it keeps the window's rounded corners.
    style.border.radius = window_radius.into();
    style
}

pub fn panel<'a, M: 'a>(
    content: Element<'a, M>,
    narrow: bool,
    viewport: Size,
    window_radius: f32,
) -> Element<'a, M> {
    let panel = container(content).padding(20);
    if narrow {
        panel
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |theme: &iad::Theme| sheet_style(theme, window_radius))
            .into()
    } else {
        panel
            .width(Length::Fixed(560.0_f32.min(viewport.width)))
            .height(Length::Fixed(
                640.0_f32.min((viewport.height - 80.0).max(160.0)),
            ))
            .style(iad::styles::container::dialog)
            .into()
    }
}
