//! Colours stored as `#rrggbb` text.

/// `#rrggbb` (or `rrggbb`) as its three channels.
pub fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 || !h.is_ascii() {
        return None;
    }
    let ch = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([ch(0)?, ch(2)?, ch(4)?])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_six_digit_hex_with_or_without_the_hash() {
        assert_eq!(parse_hex("#ff8000"), Some([255, 128, 0]));
        assert_eq!(parse_hex("FF8000"), Some([255, 128, 0]));
        assert_eq!(parse_hex(" #00ff00 "), Some([0, 255, 0]));
    }

    #[test]
    fn rejects_anything_else() {
        for bad in ["", "#fff", "#12345", "#gggggg", "red", "#ff80000", "#ééé"] {
            assert_eq!(parse_hex(bad), None, "{bad}");
        }
    }
}
