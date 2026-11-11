//! A coloured dot, and a label led by one: how a status (or any named colour) is marked.

use iced::widget::{container, row, space, text};
use iced::{Alignment, Background, Border, Color, Length};

use crate::Element;
use crate::iad::Theme;
use crate::primitives::color::parse_hex;

/// `#rrggbb` as a colour for painting.
pub fn color_from_hex(hex: &str) -> Option<Color> {
    parse_hex(hex).map(|[r, g, b]| Color::from_rgb8(r, g, b))
}

/// A filled circle `size` across.
pub fn dot<'a, M: 'a>(color: Color, size: f32) -> Element<'a, M> {
    container(space().width(size).height(size))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            border: Border::default().rounded(size / 2.0),
            ..container::Style::default()
        })
        .into()
}

/// A dot, then the name. With no colour the dot is left out.
pub fn dot_label<'a, M: 'a>(name: &str, color: Option<Color>) -> Element<'a, M> {
    let mut r = row![].spacing(6).align_y(Alignment::Center);
    if let Some(c) = color {
        r = r.push(dot(c, 10.0));
    }
    r.push(text(name.to_string()).width(Length::Shrink)).into()
}
