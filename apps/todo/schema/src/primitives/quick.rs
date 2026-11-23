//! Quick-add text: `Buy milk #home @errands due:tomorrow start:+3d` → a title, tags and dates.
//!
//! `#tag` or `@tag` adds a tag. `due:` and `start:` take `YYYY-MM-DD`, `today`, `tomorrow`,
//! `+3d` (days from today) or a weekday name (the next one). A word that does not parse stays in
//! the title, so nothing the person typed is lost.

use chrono::{Datelike, Days, NaiveDate, Weekday};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Quick {
    pub title: String,
    pub tags: Vec<String>,
    pub start: Option<NaiveDate>,
    pub due: Option<NaiveDate>,
}

fn weekday(s: &str) -> Option<Weekday> {
    s.parse().ok()
}

/// A date phrase relative to `today`.
pub fn date(s: &str, today: NaiveDate) -> Option<NaiveDate> {
    let s = s.trim().to_ascii_lowercase();
    match s.as_str() {
        "today" => return Some(today),
        "tomorrow" => return today.checked_add_days(Days::new(1)),
        _ => {}
    }
    if let Some(n) = s
        .strip_prefix('+')
        .and_then(|r| r.strip_suffix('d'))
        .and_then(|n| n.parse::<u64>().ok())
    {
        return today.checked_add_days(Days::new(n));
    }
    if let Some(w) = weekday(&s) {
        let ahead = (7 + w.num_days_from_monday() as i64
            - today.weekday().num_days_from_monday() as i64
            - 1)
            % 7
            + 1;
        return today.checked_add_days(Days::new(ahead as u64));
    }
    s.parse().ok()
}

pub fn parse(text: &str, today: NaiveDate) -> Quick {
    let mut q = Quick::default();
    let mut title = Vec::new();
    for word in text.split_whitespace() {
        if let Some(t) = word
            .strip_prefix('#')
            .or_else(|| word.strip_prefix('@'))
            .filter(|t| !t.is_empty())
        {
            q.tags.push(t.to_string());
        } else if let Some(d) = word.strip_prefix("due:").and_then(|d| date(d, today)) {
            q.due = Some(d);
        } else if let Some(d) = word.strip_prefix("start:").and_then(|d| date(d, today)) {
            q.start = Some(d);
        } else {
            title.push(word);
        }
    }
    q.title = title.join(" ");
    q
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn parses_tags_dates_and_keeps_the_rest() {
        let today = d("2026-11-10"); // a Tuesday
        let q = parse("Buy milk #home @errands due:tomorrow start:+3d", today);
        assert_eq!(q.title, "Buy milk");
        assert_eq!(q.tags, ["home", "errands"]);
        assert_eq!(
            (q.due, q.start),
            (Some(d("2026-11-11")), Some(d("2026-11-13")))
        );
        let q = parse("Call due:friday and due:soon #", today);
        assert_eq!(q.due, Some(d("2026-11-13")));
        assert_eq!(q.title, "Call and due:soon #");
        assert_eq!(
            parse("due:tuesday x", today).due,
            Some(d("2026-11-17")),
            "never today"
        );
        assert_eq!(parse("x due:2026-12-01", today).due, Some(d("2026-12-01")));
    }
}
