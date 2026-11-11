//! Colours for tags that have none: a fixed palette, and a stable pick from it by name.

/// The colours new tags are drawn from: readable as a label on both light and dark themes.
pub const TAG_PALETTE: [&str; 10] = [
    "#e66a6a", "#f0a35e", "#e0c040", "#7fc97f", "#5cc2b8", "#6aa5e8", "#8e99f0", "#a98be0",
    "#e88fc0", "#9aa0a6",
];

/// The palette colour for `name`: the same name (ignoring case) always gets the same colour.
pub fn pick<'a>(name: &str, palette: &'a [&'a str]) -> Option<&'a str> {
    if palette.is_empty() {
        return None;
    }
    // FNV-1a: tiny, and the same on every platform and run (unlike `DefaultHasher`).
    let mut h: u64 = 0xcbf29ce484222325;
    for b in name.to_lowercase().bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    Some(palette[(h % palette.len() as u64) as usize])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_always_gets_the_same_colour_whatever_its_case() {
        assert_eq!(pick("Work", &TAG_PALETTE), pick("work", &TAG_PALETTE));
        assert_eq!(pick("work", &TAG_PALETTE), pick("work", &TAG_PALETTE));
    }

    #[test]
    fn names_spread_over_the_palette() {
        let used: std::collections::HashSet<_> = [
            "a", "b", "c", "home", "work", "travel", "urgent", "ideas", "x", "y",
        ]
        .iter()
        .filter_map(|n| pick(n, &TAG_PALETTE))
        .collect();
        assert!(used.len() >= 4, "only {} colours used", used.len());
    }

    #[test]
    fn an_empty_palette_picks_nothing_and_the_palette_is_valid_hex() {
        assert_eq!(pick("x", &[]), None);
        assert!(
            TAG_PALETTE
                .iter()
                .all(|c| c.len() == 7 && c.starts_with('#'))
        );
    }
}
