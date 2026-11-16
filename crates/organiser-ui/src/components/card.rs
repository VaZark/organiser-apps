//! A note card, as in Google Keep: a rounded surface holding a title, a snippet and chips, that
//! can take a colour tint and lifts a little when hovered. The whole card is one button;
//! controls inside it stay independent.

use iced::widget::button;
use iced::{Background, Border, Color, Length};

use crate::Element;
use crate::iad::color::{color_contrast, mix};
use crate::iad::styles::container::card as card_surface;
use crate::iad::{Role, Theme};

/// How strong a card's tint is: a soft pastel wash, as in Keep, not a solid fill.
const TINT_ALPHA: f32 = 0.22;

/// The card's fill. An untinted card keeps iad's card surface; a tinted one is its tint at a soft
/// alpha. Hovering and pressing lift it: more opaque over a translucent surface,
/// lighter or darker over an opaque one, according to its contrast.
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
        mix(
            base,
            color_contrast(base, &[Color::BLACK, Color::WHITE]),
            lift,
        )
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

    fn themes() -> [Theme; 4] {
        // Only the libadwaita card colors are needed; do not depend on installed theme files
        // or install global config that could affect parallel tests.
        let config = crate::iad::theme_config::ThemeConfig::builder("card-test")
            .files(false)
            .app_text(
                r##"{
    "colors": {
        "card-bg": "#ffffff",
        "card-fg": "#000006",
    },
    "dark:colors": {
        "card-bg": "rgba(255, 255, 255, 0.08)",
        "card-fg": "#ffffff",
    },
}
"##,
            )
            .build();
        [
            Theme::dark(),
            Theme::light(),
            Theme::dark().with_theme_config(&config),
            Theme::light().with_theme_config(&config),
        ]
    }

    #[test]
    fn fixtures_cover_neutral_and_configured_card_surfaces() {
        let [
            neutral_dark,
            neutral_light,
            configured_dark,
            configured_light,
        ] = themes();
        let rest = |theme: &Theme| fill(theme, None, button::Status::Active);
        assert_eq!(rest(&neutral_dark), Color::BLACK);
        assert_eq!(rest(&neutral_light), Color::WHITE);
        assert_eq!(
            rest(&configured_dark),
            Color {
                a: 0.08,
                ..Color::WHITE
            }
        );
        assert_eq!(rest(&configured_light), Color::WHITE);
    }

    #[test]
    fn a_tint_replaces_the_surface_with_a_soft_wash_of_its_colour() {
        for theme in themes() {
            let tinted = fill(&theme, Some(RED), button::Status::Active);
            assert_eq!((tinted.r, tinted.g, tinted.b), (1.0, 0.0, 0.0));
            assert_eq!(tinted.a, TINT_ALPHA);
        }
    }

    #[test]
    fn hover_and_press_lift_neutral_and_configured_cards() {
        for theme in themes() {
            for tint in [None, Some(RED)] {
                let rest = fill(&theme, tint, button::Status::Active);
                let hover = fill(&theme, tint, button::Status::Hovered);
                let press = fill(&theme, tint, button::Status::Pressed);
                assert_ne!(hover, rest, "hover changes the fill");
                assert_ne!(press, hover, "press goes further");
                assert_eq!(fill(&theme, tint, button::Status::Disabled), rest);
                if rest.a < 0.9 {
                    assert_eq!((hover.r, hover.g, hover.b), (rest.r, rest.g, rest.b));
                    assert!(hover.a > rest.a);
                    assert!(press.a > hover.a);
                }
            }
        }
    }

    #[test]
    fn opaque_custom_surfaces_use_their_color_not_the_theme_scheme() {
        use crate::iad::color::{hex, relative_luminance};

        for mut theme in [Theme::dark(), Theme::light()] {
            for base in [Color::BLACK, hex(0x242424), Color::WHITE, hex(0xfafafa)] {
                theme.semantic.card.bg = base;
                let hover = fill(&theme, None, button::Status::Hovered);
                let press = fill(&theme, None, button::Status::Pressed);
                let luminance = relative_luminance(base);
                if color_contrast(base, &[Color::BLACK, Color::WHITE]) == Color::WHITE {
                    assert!(relative_luminance(hover) > luminance);
                    assert!(relative_luminance(press) > relative_luminance(hover));
                } else {
                    assert!(relative_luminance(hover) < luminance);
                    assert!(relative_luminance(press) < relative_luminance(hover));
                }
                assert_eq!(hover.a, 1.0);
                assert_eq!(press.a, 1.0);
            }
        }
    }
}
