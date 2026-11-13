//! A whole note as a Markdown file: the title as a first-level heading, then the body.

use super::color::Color;
use organiser_vault::front_matter::{self, FrontMatter};
use super::note::Note;
use organiser_markdown::export::export;
use organiser_markdown::parse::parse;

/// `# Title`, a blank line, and the body written back out as clean Markdown.
pub fn to_markdown(note: &Note) -> String {
    let title = note.title.replace('\n', " ");
    let body = export(&parse(&note.body));
    if body.is_empty() {
        format!("# {title}\n")
    } else {
        format!("# {title}\n\n{body}")
    }
}

/// The title and body of a Markdown file. A first-level heading on the first line is the title
/// (and is not repeated in the body); otherwise `fallback` (a file name) is. The body is kept
/// exactly as written.
pub fn from_markdown(md: &str, fallback: &str) -> (String, String) {
    let md = md.strip_prefix('\u{feff}').unwrap_or(md);
    let (first, rest) = md.split_once('\n').unwrap_or((md, ""));
    match first.strip_prefix("# ").map(str::trim) {
        Some(t) if !t.is_empty() => (t.to_string(), rest.trim_matches('\n').to_string()),
        _ => (fallback.to_string(), md.trim_matches('\n').to_string()),
    }
}

/// What a vault file says about its note (see [`parse_file`]). Folder and section are not here:
/// they are the directories the file is in. Nor is the desktop sticky: that stays with the app.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FileNote {
    /// The note it is, when the file says (an `id:` in its front matter).
    pub id: Option<i64>,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub color: Option<Color>,
    pub pinned: bool,
    pub favorite: bool,
    pub rating: Option<u8>,
    pub archived: bool,
    pub created: Option<i64>,
    /// Unix seconds, for a note in the trash.
    pub trashed: Option<i64>,
    /// Where a trashed note was (`Folder` or `Folder/Section`), to put it back.
    pub from: Option<String>,
    /// The block as read, so a key this app does not know is kept when the file is rewritten.
    pub front: Option<FrontMatter>,
}

/// A note from a Markdown file, with or without front matter. The title is the first
/// `# heading` of the body, else `fallback` (the file's name); the body is kept as written.
pub fn parse_file(text: &str, fallback: &str) -> FileNote {
    let (front, rest) = front_matter::split(text);
    let (title, body) = from_markdown(rest, fallback);
    let mut note = FileNote {
        title,
        body,
        ..FileNote::default()
    };
    if let Some(fm) = &front {
        note.id = fm.get_i64("id");
        note.tags = fm.list("tags");
        note.color = fm.get("color").and_then(|c| Color::parse(&c));
        note.pinned = fm.get_bool("pinned").unwrap_or(false);
        note.favorite = fm.get_bool("favorite").unwrap_or(false);
        note.rating = fm
            .get_i64("rating")
            .and_then(|r| u8::try_from(r).ok())
            .filter(|r| (1..=5).contains(r));
        note.archived = fm.get_bool("archived").unwrap_or(false);
        note.created = fm.get("created").and_then(|c| front_matter::parse_iso(&c));
        note.trashed = fm.get("trashed").and_then(|c| front_matter::parse_iso(&c));
        note.from = fm.get("from");
    }
    note.front = front;
    note
}

/// The file for `note`: front matter, the title as a heading, then the body as it is. `previous`
/// is the block the file had, so keys that are not this app's are kept. `from` says where a
/// trashed note was.
pub fn render_file(note: &Note, from: Option<&str>, previous: Option<&FrontMatter>) -> String {
    let mut fm = previous.cloned().unwrap_or_default();
    fm.set_raw("id", &note.id.to_string());
    if note.tags.is_empty() {
        fm.remove("tags");
    } else {
        fm.set_list("tags", &note.tags);
    }
    match note.color {
        Some(c) => fm.set("color", &c.hex()),
        None => fm.remove("color"),
    }
    for (key, on) in [
        ("pinned", note.pinned),
        ("favorite", note.favorite),
        ("archived", note.archived),
    ] {
        if on {
            fm.set_raw(key, "true");
        } else {
            fm.remove(key);
        }
    }
    match note.rating {
        Some(r) => fm.set_raw("rating", &r.to_string()),
        None => fm.remove("rating"),
    }
    fm.set_raw("created", &front_matter::iso(note.created));
    match (note.trashed, from) {
        (Some(t), _) => {
            fm.set_raw("trashed", &front_matter::iso(t));
            match from {
                Some(f) => fm.set("from", f),
                None => fm.remove("from"),
            }
        }
        _ => {
            fm.remove("trashed");
            fm.remove("from");
        }
    }
    let title = note.title.replace('\n', " ");
    let body = note.body.trim_matches('\n');
    if body.is_empty() {
        format!("{}# {title}\n", fm.render())
    } else {
        format!("{}# {title}\n\n{body}\n", fm.render())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(title: &str, body: &str) -> Note {
        Note {
            id: 1,
            folder_id: None,
            title: title.into(),
            body: body.into(),
            color: None,
            pinned: false,
            favorite: false,
            rating: None,
            sticky: false,
            archived: false,
            trashed: None,
            tags: vec![],
            created: 0,
            updated: 0,
        }
    }

    #[test]
    fn a_note_exports_as_heading_then_clean_body() {
        let md = to_markdown(&note("Trip", "* one\n* two\n"));
        assert_eq!(md, "# Trip\n\n- one\n- two\n");
        assert_eq!(to_markdown(&note("Only title", "")), "# Only title\n");
    }

    #[test]
    fn export_then_import_gives_the_note_back() {
        let n = note("Trip plan", "Pack **light**.\n\n- [ ] Tickets\n");
        let (title, body) = from_markdown(&to_markdown(&n), "x");
        assert_eq!(
            (title.as_str(), body.as_str()),
            ("Trip plan", "Pack **light**.\n\n- [ ] Tickets")
        );
    }

    #[test]
    fn a_file_without_a_title_heading_uses_the_fallback_and_keeps_everything() {
        let (t, b) = from_markdown("## Not a title\ntext\n", "notes.md");
        assert_eq!(
            (t.as_str(), b.as_str()),
            ("notes.md", "## Not a title\ntext")
        );
        let (t, _) = from_markdown("#\nbody", "f");
        assert_eq!(t, "f");
    }

    #[test]
    fn a_file_with_front_matter_round_trips() {
        let mut n = note("Trip plan", "Pack **light**.\n\n- [ ] Tickets");
        n.id = 42;
        n.tags = vec!["travel".into(), "a, b".into()];
        n.color = Some(Color::rgb(0xe8, 0xb6, 0xff));
        n.pinned = true;
        n.favorite = true;
        n.rating = Some(4);
        n.created = 1_759_305_600;
        let text = render_file(&n, None, None);
        assert!(text.starts_with("---\nid: 42\n"));
        let back = parse_file(&text, "x");
        assert_eq!(back.id, Some(42));
        assert_eq!(back.title, "Trip plan");
        assert_eq!(back.body, "Pack **light**.\n\n- [ ] Tickets");
        assert_eq!(back.tags, ["travel", "a, b"]);
        assert_eq!(back.color, Some(Color::rgb(0xe8, 0xb6, 0xff)));
        assert!(back.pinned && back.favorite && !back.archived);
        assert_eq!(back.rating, Some(4));
        assert_eq!(back.created, Some(1_759_305_600));
        // Written again from what was read, it is the same file.
        let mut again = n.clone();
        again.title = back.title.clone();
        assert_eq!(render_file(&again, None, back.front.as_ref()), text);
    }

    #[test]
    fn a_plain_file_has_no_front_matter_and_keeps_everything() {
        let f = parse_file("# Title\n\nhello\n", "stem");
        assert_eq!((f.id, f.title.as_str(), f.body.as_str()), (None, "Title", "hello"));
        assert!(f.front.is_none());
        let f = parse_file("just text\n", "stem");
        assert_eq!((f.title.as_str(), f.body.as_str()), ("stem", "just text"));
    }

    #[test]
    fn keys_of_other_tools_are_kept_and_cleared_flags_leave() {
        let f = parse_file("---\nid: 1\npinned: true\naliases: [x]\n---\n# T\n", "s");
        let mut n = note("T", "");
        n.id = 1;
        let text = render_file(&n, None, f.front.as_ref());
        assert!(text.contains("aliases: [x]"));
        assert!(!text.contains("pinned"), "the note is no longer pinned");
    }

    #[test]
    fn a_trashed_note_remembers_where_it_was() {
        let mut n = note("Gone", "b");
        n.id = 3;
        n.trashed = Some(1_759_305_600);
        let text = render_file(&n, Some("Work/Design"), None);
        let back = parse_file(&text, "x");
        assert_eq!(back.trashed, Some(1_759_305_600));
        assert_eq!(back.from.as_deref(), Some("Work/Design"));
    }
}
