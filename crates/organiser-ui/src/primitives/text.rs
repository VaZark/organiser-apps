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

/// The first line of Markdown that says something, as plain text for a one-line preview: without
/// its heading marks, quote mark, bullet, number or task box, and without emphasis and code
/// marks, cut to `max` characters with an ellipsis. Empty text gives an empty line.
pub fn plain_line(markdown: &str, max: usize) -> String {
    let line = markdown
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.chars().all(|c| matches!(c, '-' | '*' | '_' | '=' | '`')))
        .unwrap_or_default();
    let mut s = line.trim_start_matches(['#', '>']).trim_start();
    // A bullet or a number, then a task box.
    if let Some(rest) = s
        .strip_prefix("- ")
        .or_else(|| s.strip_prefix("* "))
        .or_else(|| s.strip_prefix("+ "))
    {
        s = rest.trim_start();
    } else if let Some((n, rest)) = s.split_once(". ")
        && !n.is_empty()
        && n.chars().all(|c| c.is_ascii_digit())
    {
        s = rest.trim_start();
    }
    for boxed in ["[ ] ", "[x] ", "[X] "] {
        if let Some(rest) = s.strip_prefix(boxed) {
            s = rest;
        }
    }
    let plain: String = s.chars().filter(|c| !matches!(c, '*' | '`' | '~')).collect();
    ellipsize(plain.trim(), max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_preview_line_loses_its_markdown_marks() {
        assert_eq!(plain_line("## Day 1\nmore", 80), "Day 1");
        assert_eq!(plain_line("- [x] Milk\n- [ ] Eggs", 80), "Milk");
        assert_eq!(plain_line("> The best way", 80), "The best way");
        assert_eq!(plain_line("3. Third item", 80), "Third item");
        assert_eq!(plain_line("A habit tracker that *never* nags.", 80), "A habit tracker that never nags.");
        assert_eq!(plain_line("Network: `casa-5g`.", 80), "Network: casa-5g.");
        // A rule or blank lines first are skipped; nothing at all is empty.
        assert_eq!(plain_line("\n---\nText", 80), "Text");
        assert_eq!(plain_line("", 80), "");
        assert_eq!(plain_line("abcdefghij", 5), "abcd…");
    }

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
