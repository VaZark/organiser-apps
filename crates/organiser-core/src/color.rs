//! Custom item colours: any RGB value, stored as `#rrggbb`.
//!
//! DECISION (user, 2026-11-23): every app's items (notes, tasks, recipes) get a colour of their own
//! with the same rules as Notes: any `#rrggbb` (or `#rgb`), the [`PRESETS`] as quick picks, none by
//! default, tinting the card and the page. One picker (`organiser-qt` `ColorPicker.qml`) serves all
//! three. Revisit if an app needs a different palette.

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Quick picks offered next to the custom picker. Not a limit: any [`Color`] is valid.
/// What an item with no colour of its own, and none above it, is drawn in.
pub const DEFAULT_HEX: &str = "#9e9e9e";

pub const PRESETS: &[(&str, &str)] = &[
    ("Red", "#e66a6a"),
    ("Orange", "#f0a35e"),
    ("Yellow", "#f2d45c"),
    ("Green", "#7fc97f"),
    ("Teal", "#5cc2b8"),
    ("Blue", "#6aa5e8"),
    ("Purple", "#a98be0"),
    ("Pink", "#e88fc0"),
];

/// [`PRESETS`] as JSON for a GUI: `[{"name":"Red","hex":"#e66a6a"}, ...]`.
pub fn presets_json() -> String {
    let items: Vec<String> = PRESETS
        .iter()
        .map(|(name, hex)| format!("{{\"name\":\"{name}\",\"hex\":\"{hex}\"}}"))
        .collect();
    format!("[{}]", items.join(","))
}

/// A colour as typed: empty is none, anything else must be `#rrggbb` or `#rgb` (any case) and is
/// returned as `#rrggbb`. The error is a sentence for the person.
pub fn parse_input(s: &str) -> Result<Option<Color>, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    Color::parse(s)
        .map(Some)
        .ok_or_else(|| format!("{s} is not a colour. Use a form like #6aa5e8."))
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Accepts `#rrggbb`, `rrggbb`, `#rgb` and `rgb`, in any case.
    pub fn parse(s: &str) -> Option<Self> {
        let h = s.trim().trim_start_matches('#');
        if !h.is_ascii() {
            return None;
        }
        let pair = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
        let nib = |i: usize| {
            u8::from_str_radix(h.get(i..i + 1)?, 16)
                .ok()
                .map(|n| n * 17)
        };
        match h.len() {
            6 => Some(Self::rgb(pair(0)?, pair(2)?, pair(4)?)),
            3 => Some(Self::rgb(nib(0)?, nib(1)?, nib(2)?)),
            _ => None,
        }
    }

    pub fn hex(self) -> String {
        self.to_string()
    }

    /// Whether dark text reads better on this colour (WCAG relative luminance).
    pub fn prefers_dark_text(self) -> bool {
        let lin = |c: u8| {
            let c = f64::from(c) / 255.0;
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(self.r) + 0.7152 * lin(self.g) + 0.0722 * lin(self.b) > 0.179
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_long_and_short_forms() {
        assert_eq!(Color::parse("#FF8000"), Some(Color::rgb(255, 128, 0)));
        assert_eq!(Color::parse("ff8000"), Some(Color::rgb(255, 128, 0)));
        assert_eq!(Color::parse("#f80"), Some(Color::rgb(255, 136, 0)));
    }

    #[test]
    fn presets_json_lists_every_preset_and_input_is_checked() {
        let j = presets_json();
        assert!(j.starts_with("[{\"name\":\"Red\",\"hex\":\"#e66a6a\"}"));
        assert_eq!(j.matches("\"hex\"").count(), PRESETS.len());
        assert_eq!(parse_input(" "), Ok(None));
        assert_eq!(parse_input("#F80"), Ok(Some(Color::rgb(255, 136, 0))));
        assert!(parse_input("red").is_err());
    }

    #[test]
    fn rejects_garbage() {
        for bad in ["", "#12", "#12345", "#gggggg", "red", "#ééé"] {
            assert_eq!(Color::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn hex_round_trips_and_presets_are_valid() {
        for (_, h) in PRESETS {
            assert_eq!(Color::parse(h).unwrap().hex(), *h);
        }
    }

    #[test]
    fn text_contrast_follows_luminance() {
        assert!(Color::rgb(255, 255, 255).prefers_dark_text());
        assert!(!Color::rgb(0, 0, 0).prefers_dark_text());
    }
}
