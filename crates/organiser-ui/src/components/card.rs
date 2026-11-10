//! A note card, as in Google Keep: a rounded surface holding a title, a snippet and chips, that
//! can take a colour tint and lifts a little when hovered. The whole card is one button;
//! controls inside it stay independent.

use iced::widget::button;
use iced::{Background, Border, Color, Length};

use crate::Element;
use crate::iad::color::mix;
use crate::iad::styles::container::card as card_surface;
use crate::iad::{Role, Theme};

/// How strong a card's tint is: a soft pastel wash, as in Keep, not a solid fill.
const TINT_ALPHA: f32 = 0.22;

/// The card's fill. An untinted card keeps iad's card surface; a tinted one is its tint at a soft
/// alpha. Hovering and pressing lift it: more opaque over a translucent surface (dark themes),
/// darker over an opaque one (light themes).
fn fill(theme: &Theme, tint: Option<Color>, status: button::Status) -> Color {
    let surface = match card_surface(theme).background {
        Some(Background::Color(c)) => c,
        _ => Color::TRANSPARENT,
    };
    let base = tint.map_or(surface, |t| Color { a: TINT_ALPHA, ..t });
    let lift = match status {
        button::Status::Hovered => 0.06,
        button::Status::Pressed => 0.10,
        _ => 0.0,
    };
    if lift == 0.0 {
        base
    } else if base.a < 0.9 {
        Color {
            a: (base.a + lift).min(1.0),
            ..base
        }
    } else {
        mix(base, Color::BLACK, lift)
    }
}

pub fn card<'a, M: Clone + 'a>(
    content: impl Into<Element<'a, M>>,
    tint: Option<Color>,
    selected: bool,
    on_press: M,
) -> Element<'a, M> {
    button(content)
        .width(Length::Fill)
        .padding(12)
        .on_press(on_press)
        .style(move |theme: &Theme, status| {
            let surface = card_surface(theme);
            let mut border = surface.border;
            if selected {
                border = Border {
                    color: theme.semantic.swatch(Role::Accent).color,
                    width: 2.0,
                    ..border
                };
            }
            button::Style {
                background: Some(Background::Color(fill(theme, tint, status))),
                text_color: surface
                    .text_color
                    .unwrap_or(theme.semantic.swatch(Role::Neutral).color),
                border,
                shadow: surface.shadow,
                ..button::Style::default()
            }
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Color = Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    #[test]
    fn a_tint_replaces_the_surface_with_a_soft_wash_of_its_colour() {
        for theme in [Theme::dark(), Theme::light()] {
            let tinted = fill(&theme, Some(RED), button::Status::Active);
            assert_eq!((tinted.r, tinted.g, tinted.b), (1.0, 0.0, 0.0));
            assert_eq!(tinted.a, TINT_ALPHA);
        }
    }

    #[test]
    fn hover_and_press_lift_the_card_on_both_themes() {
        for theme in [Theme::dark(), Theme::light()] {
            for tint in [None, Some(RED)] {
                let rest = fill(&theme, tint, button::Status::Active);
                let hover = fill(&theme, tint, button::Status::Hovered);
                let press = fill(&theme, tint, button::Status::Pressed);
                assert_ne!(hover, rest, "hover changes the fill");
                assert_ne!(press, hover, "press goes further");
            }
        }
    }
}
