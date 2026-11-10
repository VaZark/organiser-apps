//! A list row that is one big button: the whole surface answers hover and press, and a selected
//! row takes a soft accent tint. Controls inside it (a checkbox, a fold arrow) stay independent.

use iced::widget::button;
use iced::{Background, Color, Length};

use crate::Element;
use crate::iad::{Role, Theme};

/// The tint of a row: `selected` is the soft accent, hovering and pressing darken either state.
fn tint(theme: &Theme, selected: bool, status: button::Status) -> Option<Background> {
    let (base, rest, hover, press) = if selected {
        (theme.semantic.swatch(Role::Accent).bg, 0.18, 0.26, 0.32)
    } else {
        (theme.semantic.swatch(Role::Neutral).color, 0.0, 0.07, 0.12)
    };
    let alpha = match status {
        button::Status::Hovered => hover,
        button::Status::Pressed => press,
        _ => rest,
    };
    (alpha > 0.0).then_some(Background::Color(Color { a: alpha, ..base }))
}

pub fn selectable<'a, M: Clone + 'a>(
    content: impl Into<Element<'a, M>>,
    selected: bool,
    on_press: M,
) -> Element<'a, M> {
    button(content)
        .width(Length::Fill)
        .padding(0)
        .on_press(on_press)
        .style(move |theme: &Theme, status| button::Style {
            background: tint(theme, selected, status),
            text_color: theme.semantic.swatch(Role::Neutral).color,
            ..button::Style::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(selected: bool, status: button::Status) -> f32 {
        match tint(&Theme::dark(), selected, status) {
            Some(Background::Color(c)) => c.a,
            _ => 0.0,
        }
    }

    #[test]
    fn the_whole_row_answers_hover_and_press() {
        use button::Status::*;
        // An idle row has no background; hover and press add one, press more than hover.
        assert_eq!(alpha(false, Active), 0.0);
        assert!(alpha(false, Hovered) > 0.0);
        assert!(alpha(false, Pressed) > alpha(false, Hovered));
    }

    #[test]
    fn a_selected_row_is_tinted_at_rest_and_darkens_on_hover() {
        use button::Status::*;
        assert!(alpha(true, Active) > 0.0);
        assert!(alpha(true, Hovered) > alpha(true, Active));
        assert!(
            alpha(true, Active) > alpha(false, Hovered),
            "selection reads stronger than hover"
        );
    }
}
