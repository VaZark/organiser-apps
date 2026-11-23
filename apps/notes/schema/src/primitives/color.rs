//! Custom card colours: any RGB value, stored as `#rrggbb`.

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Quick picks offered next to the custom picker. Not a limit: any [`Color`] is valid.
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
