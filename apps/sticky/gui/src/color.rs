//! Sticky colours: parsing, and which text colour reads on a given paper.

use iced::Color;

pub fn parse(hex: &str) -> Option<Color> {
    let h = hex.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(h, 16).ok()?;
    Some(Color::from_rgb8((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

/// The paper for an optional stored colour; `default` (a `#rrggbb`) is for a sticky with none.
pub fn paper(stored: Option<&str>, default: &str) -> Color {
    stored
        .and_then(parse)
        .or_else(|| parse(default))
        .unwrap_or(Color::from_rgb8(0xff, 0xf5, 0x9d))
}

/// Dark ink on light paper, light ink on dark paper.
pub fn ink(paper: Color) -> Color {
    let lum = 0.2126 * paper.r + 0.7152 * paper.g + 0.0722 * paper.b;
    if lum > 0.5 {
        Color::from_rgb8(0x2b, 0x2b, 0x2b)
    } else {
        Color::from_rgb8(0xf5, 0xf5, 0xf5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_and_falls_back_to_the_default_paper() {
        assert_eq!(parse("#ff0000"), Some(Color::from_rgb8(255, 0, 0)));
        assert_eq!(parse("ff0000"), None);
        assert_eq!(parse("#ff00"), None);
        assert_eq!(paper(None, "#00ff00"), parse("#00ff00").unwrap());
        assert_eq!(paper(Some("nope"), "#00ff00"), parse("#00ff00").unwrap());
        assert_eq!(paper(Some("#0000ff"), "#00ff00"), parse("#0000ff").unwrap());
    }

    #[test]
    fn ink_contrasts_with_the_paper() {
        assert_eq!(
            ink(parse("#fff59d").unwrap()),
            Color::from_rgb8(0x2b, 0x2b, 0x2b)
        );
        assert_eq!(ink(Color::BLACK), Color::from_rgb8(0xf5, 0xf5, 0xf5));
    }
}
