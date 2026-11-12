//! A colour to pick or type, shown as a round swatch: pressing it opens the picker, with a few
//! suggested colours. The colour of a list, an epic, an entry.

use iced::Color;

use crate::Element;
use crate::components::dot::{color_from_hex, dot};
use crate::iad::prelude::*;
use crate::iad::primitives::color_picker::Hsva;

/// The colours suggested in the picker beside the free choice.
pub const SWATCHES: [&str; 8] = [
    "#8e99a8", "#6aa5e8", "#7fc97f", "#f2d45c", "#f0a35e", "#e0787a", "#a98be0", "#e88fc0",
];

/// What an unset colour looks like.
pub const UNSET: Color = Color::from_rgb(0x9a as f32 / 255.0, 0xa0 as f32 / 255.0, 0xa6 as f32 / 255.0);

/// The picker's value for a colour.
pub fn hsva(c: Color) -> Hsva {
    Hsva::from_rgb8(
        (c.r * 255.0).round() as u8,
        (c.g * 255.0).round() as u8,
        (c.b * 255.0).round() as u8,
    )
}

/// `#rrggbb` for a picked colour.
pub fn hex(h: Hsva) -> String {
    let [r, g, b] = h.to_rgb8();
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// A colour swatch `size` across for `current` (`#rrggbb`, or none), with `typed` the text being
/// typed in the picker if it differs from `current`.
pub fn color_swatch<'a, M: Clone + 'a>(
    current: Option<&'a str>,
    typed: Option<&'a str>,
    size: f32,
    on_text: impl Fn(String) -> M + 'a,
    on_pick: impl Fn(Hsva) -> M + 'a,
) -> Element<'a, M> {
    let hex_now = current.unwrap_or("#9aa0a6");
    let rgb = color_from_hex(hex_now).unwrap_or(UNSET);
    color_swatch_rgb(rgb, typed.unwrap_or(hex_now), size, on_text, on_pick)
}

/// Like [`color_swatch`], for a colour held as a value, with `typed` the text the picker shows.
pub fn color_swatch_rgb<'a, M: Clone + 'a>(
    rgb: Color,
    typed: &'a str,
    size: f32,
    on_text: impl Fn(String) -> M + 'a,
    on_pick: impl Fn(Hsva) -> M + 'a,
) -> Element<'a, M> {
    color_swatch_with(rgb, typed, size, SWATCHES, on_text, on_pick)
}

/// Like [`color_swatch_rgb`], with the suggested colours (`#rrggbb`) the app chooses.
pub fn color_swatch_with<'a, M: Clone + 'a>(
    rgb: Color,
    typed: &'a str,
    size: f32,
    swatches: impl IntoIterator<Item = &'a str>,
    on_text: impl Fn(String) -> M + 'a,
    on_pick: impl Fn(Hsva) -> M + 'a,
) -> Element<'a, M> {
    ColorInput::new(hsva(rgb), typed, on_text, on_pick)
        .swatches(swatches)
        .trigger(dot(rgb, size))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colour_round_trips_through_the_picker_and_back_to_hex() {
        let c = color_from_hex("#6aa5e8").unwrap();
        assert_eq!(hex(hsva(c)), "#6aa5e8");
    }
}
