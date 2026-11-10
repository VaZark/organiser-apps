//! Fitting text into a fixed-size space.

/// `s` cut to at most `max` characters, ending in `…` when anything was cut. Counts characters,
/// not bytes, so accented and non-Latin text is never split in the middle.
pub fn ellipsize(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if s.chars().count() <= max {
        return s.to_string();
    }
    let keep = max.saturating_sub(1);
    let mut out: String = s.chars().take(keep).collect();
    // A cut that lands after a space would leave "word …"; drop the space.
    while out.ends_with(char::is_whitespace) {
        out.pop();
    }
    out.push('…');
    out
}

/// The start of a longer text, for a card: at most `lines` non-empty lines and `max` characters in
/// all, ending in `…` when anything was left out. Empty text gives an empty snippet.
pub fn snippet(s: &str, lines: usize, max: usize) -> String {
    let picked: Vec<&str> = s.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let cut_lines = picked.len() > lines;
    let joined = picked
        .into_iter()
        .take(lines)
        .collect::<Vec<_>>()
        .join("\n");
    let out = ellipsize(&joined, max);
    if cut_lines && !out.ends_with('…') {
        format!("{out}…")
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_text_is_untouched_and_long_text_ends_in_an_ellipsis() {
        assert_eq!(ellipsize("home", 24), "home");
        assert_eq!(ellipsize("exactly ten", 11), "exactly ten");
        assert_eq!(ellipsize("abcdefghij", 5), "abcd…");
        assert_eq!(ellipsize("abcdefghij", 5).chars().count(), 5);
    }

    #[test]
    fn counts_characters_and_does_not_leave_a_dangling_space() {
        assert_eq!(ellipsize("Été à Lisbonne", 6), "Été à…");
        assert_eq!(ellipsize("ab cd ef", 4), "ab…");
        assert_eq!(ellipsize("日本語のタグです", 4), "日本語…");
        assert_eq!(ellipsize("x", 0), "");
    }

    #[test]
    fn snippets_take_a_few_lines_and_say_when_there_is_more() {
        assert_eq!(snippet("", 3, 100), "");
        assert_eq!(snippet("one line", 3, 100), "one line");
        assert_eq!(snippet("a\n\n  b  \nc", 3, 100), "a\nb\nc");
        assert_eq!(snippet("a\nb\nc\nd", 3, 100), "a\nb\nc…");
        assert_eq!(snippet("abcdefghij", 3, 5), "abcd…");
    }
}
