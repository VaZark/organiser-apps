//! The metadata block at the top of a Markdown file: `---`, `key: value` lines, `---`, as
//! Obsidian, Jekyll and QOwnNotes write it. A small subset of YAML (flat keys; a value is a
//! scalar, an inline `[a, b]` list or `- a` lines), with no dependency.
//!
//! The block keeps every line it read, in order, so a key this app does not know (another tool's)
//! comes back out unchanged when the file is written again.

/// One key and the lines it was written on: the `key: value` line, then any `- item` or indented
/// lines that continue it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    key: String,
    lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrontMatter {
    entries: Vec<Entry>,
}

/// The front matter of `text` (if it starts with one) and the rest of it. A block that is never
/// closed is not front matter: the whole text is the body.
pub fn split(text: &str) -> (Option<FrontMatter>, &str) {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let Some(rest) = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
    else {
        return (None, text);
    };
    let mut at = 0;
    for line in rest.split_inclusive('\n') {
        let bare = line.trim_end_matches(['\n', '\r']);
        if bare.trim_end() == "---" || bare.trim_end() == "..." {
            let block = &rest[..at];
            let body = &rest[at + line.len()..];
            return (Some(FrontMatter::parse(block)), body);
        }
        at += line.len();
    }
    (None, text)
}

impl FrontMatter {
    fn parse(block: &str) -> Self {
        let mut entries: Vec<Entry> = Vec::new();
        for line in block.lines() {
            match key_of(line) {
                Some(key) => entries.push(Entry {
                    key,
                    lines: vec![line.to_string()],
                }),
                None => match entries.last_mut() {
                    Some(e) => e.lines.push(line.to_string()),
                    // Text before any key: kept, under no key.
                    None => entries.push(Entry {
                        key: String::new(),
                        lines: vec![line.to_string()],
                    }),
                },
            }
        }
        Self { entries }
    }

    /// The block as written to a file: fences included, and a newline after the last.
    pub fn render(&self) -> String {
        let mut out = String::from("---\n");
        for e in &self.entries {
            for l in &e.lines {
                out.push_str(l);
                out.push('\n');
            }
        }
        out.push_str("---\n");
        out
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn entry(&self, key: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.key == key)
    }

    /// The scalar `key: value`, unquoted; `None` when the key is missing or has no value.
    pub fn get(&self, key: &str) -> Option<String> {
        let e = self.entry(key)?;
        let rest = e.lines[0].split_once(':')?.1;
        let v = unquote(rest.trim());
        (!v.is_empty()).then_some(v)
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.get(key)?.to_ascii_lowercase().as_str() {
            "true" | "yes" => Some(true),
            "false" | "no" => Some(false),
            _ => None,
        }
    }

    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key)?.parse().ok()
    }

    /// The list under `key`: `[a, b]` on the line, `- a` lines under it, or a lone scalar.
    pub fn list(&self, key: &str) -> Vec<String> {
        let Some(e) = self.entry(key) else {
            return Vec::new();
        };
        let first = e.lines[0].split_once(':').map_or("", |p| p.1).trim();
        if let Some(inner) = first.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            return split_items(inner);
        }
        if !first.is_empty() {
            // `tags: a, b` is what some tools write.
            return split_items(first);
        }
        e.lines[1..]
            .iter()
            .filter_map(|l| l.trim_start().strip_prefix("- "))
            .map(|s| unquote(s.trim()))
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Sets `key` to the already-formatted `raw` (a number, a boolean), keeping its place.
    pub fn set_raw(&mut self, key: &str, raw: &str) {
        self.put(key, vec![format!("{key}: {raw}")]);
    }

    /// Sets `key` to the text `value`, quoted when it needs to be.
    pub fn set(&mut self, key: &str, value: &str) {
        self.set_raw(key, &quote(value));
    }

    pub fn set_list(&mut self, key: &str, items: &[String]) {
        let inner: Vec<String> = items.iter().map(|s| quote_item(s)).collect();
        self.put(key, vec![format!("{key}: [{}]", inner.join(", "))]);
    }

    pub fn remove(&mut self, key: &str) {
        self.entries.retain(|e| e.key != key);
    }

    fn put(&mut self, key: &str, lines: Vec<String>) {
        match self.entries.iter_mut().find(|e| e.key == key) {
            Some(e) => e.lines = lines,
            None => self.entries.push(Entry {
                key: key.to_string(),
                lines,
            }),
        }
    }
}

/// `key` of a line that starts one: letters, digits, `_` or `-`, then `:`.
fn key_of(line: &str) -> Option<String> {
    let (k, _) = line.split_once(':')?;
    (!k.is_empty()
        && k.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'))
    .then(|| k.to_string())
}

/// A value as it is written: in double quotes when a bare one would be read as something else.
pub fn quote(s: &str) -> String {
    let plain = !s.is_empty()
        && s == s.trim()
        && !s.contains(": ")
        && !s.contains(" #")
        && !s.contains(['\n', '"', '\\'])
        && !s.starts_with(['[', ']', '{', '}', '&', '*', '!', '|', '>', '\'', '%', '@', '`', '#', ',', '?'])
        && !s.starts_with("- ")
        && !s.ends_with(':')
        && !is_reserved(s);
    if plain {
        return s.to_string();
    }
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// As [`quote`], and also when it contains a comma or bracket, which would split an inline list.
fn quote_item(s: &str) -> String {
    if s.contains([',', '[', ']']) {
        let q = quote(s);
        if q.starts_with('"') {
            return q;
        }
        return format!("\"{s}\"");
    }
    quote(s)
}

/// Words and numbers a reader would not take for text.
fn is_reserved(s: &str) -> bool {
    matches!(
        s.to_ascii_lowercase().as_str(),
        "true" | "false" | "yes" | "no" | "null" | "~" | "on" | "off"
    ) || s.parse::<f64>().is_ok()
}

pub fn unquote(s: &str) -> String {
    let s = s.trim();
    if let Some(inner) = s.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        let mut out = String::new();
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => out.push('\n'),
                    Some(o) => out.push(o),
                    None => out.push('\\'),
                }
            } else {
                out.push(c);
            }
        }
        return out;
    }
    if let Some(inner) = s.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')) {
        return inner.replace("''", "'");
    }
    // A comment after a bare value.
    match s.find(" #") {
        Some(i) => s[..i].trim_end().to_string(),
        None => s.to_string(),
    }
}

/// `a, "b, c", d` as items, quotes respected.
fn split_items(s: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut cur = String::new();
    let mut quoted: Option<char> = None;
    let mut escaped = false;
    for c in s.chars() {
        match quoted {
            Some(q) => {
                cur.push(c);
                if escaped {
                    escaped = false;
                } else if c == '\\' && q == '"' {
                    escaped = true;
                } else if c == q {
                    quoted = None;
                }
            }
            None => match c {
                ',' => {
                    items.push(std::mem::take(&mut cur));
                }
                '"' | '\'' => {
                    quoted = Some(c);
                    cur.push(c);
                }
                c => cur.push(c),
            },
        }
    }
    items.push(cur);
    items
        .iter()
        .map(|i| unquote(i))
        .filter(|i| !i.is_empty())
        .collect()
}

// ---- times: `2026-10-01T08:00:00Z` ---------------------------------------------------------

/// Unix seconds as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
pub fn iso(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// The Unix seconds of `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS` with an optional `Z` (read as UTC).
pub fn parse_iso(s: &str) -> Option<i64> {
    let s = s.trim().trim_end_matches('Z');
    let (date, time) = s.split_once(['T', ' ']).unwrap_or((s, "00:00:00"));
    let mut d = date.split('-');
    let (y, m, day): (i64, i64, i64) = (
        d.next()?.parse().ok()?,
        d.next()?.parse().ok()?,
        d.next()?.parse().ok()?,
    );
    if d.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&day) {
        return None;
    }
    let mut t = time.split(':');
    let (h, mi, se): (i64, i64, i64) = (
        t.next()?.parse().ok()?,
        t.next().unwrap_or("0").parse().ok()?,
        t.next().unwrap_or("0").split('.').next()?.parse().ok()?,
    );
    Some(days_from_civil(y, m, day) * 86_400 + h * 3600 + mi * 60 + se)
}

// Howard Hinnant's civil-date algorithms.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_is_split_off_the_body() {
        let (fm, body) = split("---\nid: 4\ntags: [a, b]\n---\n# T\n\ntext\n");
        let fm = fm.unwrap();
        assert_eq!(fm.get_i64("id"), Some(4));
        assert_eq!(fm.list("tags"), ["a", "b"]);
        assert_eq!(body, "# T\n\ntext\n");
    }

    #[test]
    fn no_block_or_an_unclosed_one_is_all_body() {
        assert_eq!(split("# T\n").0, None);
        let text = "---\nid: 4\nno end\n";
        let (fm, body) = split(text);
        assert!(fm.is_none());
        assert_eq!(body, text);
        // A rule in the middle of a note is not front matter.
        assert!(split("text\n---\nid: 1\n---\n").0.is_none());
    }

    #[test]
    fn a_byte_order_mark_and_crlf_are_fine() {
        let (fm, body) = split("\u{feff}---\r\nid: 2\r\n---\r\nbody");
        assert_eq!(fm.unwrap().get_i64("id"), Some(2));
        assert_eq!(body, "body");
    }

    #[test]
    fn lists_in_every_spelling() {
        let (fm, _) = split("---\na: [x, \"y, z\"]\nb:\n  - one\n  - 'two'\nc: p, q\nd: lone\n---\n");
        let fm = fm.unwrap();
        assert_eq!(fm.list("a"), ["x", "y, z"]);
        assert_eq!(fm.list("b"), ["one", "two"]);
        assert_eq!(fm.list("c"), ["p", "q"]);
        assert_eq!(fm.list("d"), ["lone"]);
        assert!(fm.list("missing").is_empty());
    }

    #[test]
    fn values_that_would_be_misread_are_quoted_and_come_back() {
        for s in [
            "plain", "a: b", "# not a comment", "with \"quotes\"", "back\\slash", " padded ", "true",
            "42", "", "- dash", "[x]", "multi\nline", "ends:",
        ] {
            let mut fm = FrontMatter::default();
            fm.set("k", s);
            let (back, _) = split(&format!("{}x", fm.render()));
            let got = back.unwrap().get("k");
            // An empty value has none.
            assert_eq!(got.unwrap_or_default(), s, "{s:?}");
        }
    }

    #[test]
    fn items_with_commas_survive_a_list() {
        let mut fm = FrontMatter::default();
        let items = vec!["a, b".to_string(), "c".to_string(), "x]y".to_string()];
        fm.set_list("tags", &items);
        let (back, _) = split(&format!("{}x", fm.render()));
        assert_eq!(back.unwrap().list("tags"), items);
    }

    #[test]
    fn unknown_keys_and_their_order_are_kept() {
        let src = "---\nzeta: 1\nid: 7\nlinks:\n  - a\n  - b\ncustom: \"x: y\"\n---\n";
        let (fm, _) = split(src);
        let mut fm = fm.unwrap();
        fm.set_raw("id", "8");
        fm.set_raw("pinned", "true");
        assert_eq!(
            fm.render(),
            "---\nzeta: 1\nid: 8\nlinks:\n  - a\n  - b\ncustom: \"x: y\"\npinned: true\n---\n"
        );
        fm.remove("zeta");
        assert!(!fm.render().contains("zeta"));
    }

    #[test]
    fn a_comment_after_a_bare_value_is_dropped() {
        let (fm, _) = split("---\nid: 3 # the id\ncolor: \"#aabbcc\"\n---\n");
        let fm = fm.unwrap();
        assert_eq!(fm.get_i64("id"), Some(3));
        assert_eq!(fm.get("color").as_deref(), Some("#aabbcc"));
    }

    #[test]
    fn times_round_trip() {
        for t in [0, 1_759_305_600, 1_000_000_000, 4_102_444_799, -86_400] {
            assert_eq!(parse_iso(&iso(t)), Some(t), "{t}");
        }
        assert_eq!(iso(1_759_305_600), "2025-10-01T08:00:00Z");
        assert_eq!(parse_iso("2025-10-01"), Some(1_759_276_800));
        assert_eq!(parse_iso("nonsense"), None);
        assert_eq!(parse_iso("2025-13-01"), None);
    }
}
