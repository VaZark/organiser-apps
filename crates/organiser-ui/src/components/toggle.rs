//! An icon that is on or off. On, it takes the secondary (gray) fill rather than the accent
//! colour, so a row of toggles in a header stays quiet and the accent is kept for the one main
//! action. Off, it is a flat icon button. Hover and press darken either state.

use std::rc::Rc;

use iced::widget::{button, container};
use iced::{Background, Border, Color, Length};

use crate::Element;
use crate::iad::prelude::{TooltipPosition, tooltip};
use crate::iad::{IconSource, Metrics, Role, Theme};

/// The fill of a toggle: `active` is the resting secondary gray, and hover and press add to it.
fn fill(theme: &Theme, active: bool, status: button::Status) -> Option<Background> {
    let fg = theme.semantic.swatch(Role::Neutral).color;
    let alpha = match (active, status) {
        (_, button::Status::Disabled) => 0.0,
        (true, button::Status::Hovered) => 0.22,
        (true, button::Status::Pressed) => 0.28,
        (true, _) => 0.14,
        (false, button::Status::Hovered) => 0.08,
        (false, button::Status::Pressed) => 0.14,
        (false, _) => 0.0,
    };
    (alpha > 0.0).then_some(Background::Color(Color { a: alpha, ..fg }))
}

pub struct IconToggle<'a, M> {
    icon: IconSource,
    active: bool,
    tooltip: Option<String>,
    disabled: bool,
    on_toggle: Option<Rc<dyn Fn(bool) -> M + 'a>>,
}

impl<'a, M: 'a> IconToggle<'a, M> {
    pub fn new(icon: impl Into<IconSource>, active: bool) -> Self {
        Self {
            icon: icon.into(),
            active,
            tooltip: None,
            disabled: false,
            on_toggle: None,
        }
    }
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    /// Pressed: gets the state the toggle would take.
    pub fn on_toggle(mut self, f: impl Fn(bool) -> M + 'a) -> Self {
        self.on_toggle = Some(Rc::new(f));
        self
    }
}

impl<'a, M: Clone + 'a> From<IconToggle<'a, M>> for Element<'a, M> {
    fn from(t: IconToggle<'a, M>) -> Self {
        let m = Metrics::desktop();
        let active = t.active;
        let press = if t.disabled {
            None
        } else {
            t.on_toggle.as_ref().map(|f| f(!active))
        };
        let b =
            button(container(t.icon.view(m.icon_size)).center(Length::Fixed(m.button_min_height)))
                .padding(0)
                .on_press_maybe(press)
                .style(move |theme: &Theme, status| button::Style {
                    background: fill(theme, active, status),
                    text_color: theme.semantic.swatch(Role::Neutral).color,
                    border: Border::default().rounded(theme.metrics.radius_m),
                    ..button::Style::default()
                });
        match t.tooltip {
            Some(text) => tooltip(b, text, TooltipPosition::Bottom),
            None => b.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(active: bool, status: button::Status) -> f32 {
        match fill(&Theme::dark(), active, status) {
            Some(Background::Color(c)) => c.a,
            _ => 0.0,
        }
    }

    #[test]
    fn an_active_toggle_is_a_gray_fill_and_an_idle_one_is_flat() {
        use button::Status::*;
        assert_eq!(alpha(false, Active), 0.0);
        assert!(alpha(true, Active) > 0.0);
        assert!(alpha(true, Hovered) > alpha(true, Active));
        assert!(alpha(false, Hovered) > 0.0 && alpha(false, Hovered) < alpha(true, Active));
        assert_eq!(alpha(true, Disabled), 0.0, "a disabled toggle has no fill");
    }

    #[test]
    fn the_fill_is_the_foreground_gray_not_the_accent() {
        let t = Theme::dark();
        let Some(Background::Color(c)) = fill(&t, true, button::Status::Active) else {
            panic!("no fill")
        };
        let neutral = t.semantic.swatch(Role::Neutral).color;
        let accent = t.semantic.swatch(Role::Accent).color;
        assert_eq!((c.r, c.g, c.b), (neutral.r, neutral.g, neutral.b));
        assert_ne!((c.r, c.g, c.b), (accent.r, accent.g, accent.b));
    }
}
