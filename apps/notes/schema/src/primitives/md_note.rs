//! A whole note as a Markdown file: the title as a first-level heading, then the body.

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

#[cfg(test)]
mod tests {
    use super::*;

    fn note(title: &str, body: &str) -> Note {
        Note {
            id: 1,
            title: title.into(),
            body: body.into(),
            color: None,
            pinned: false,
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
}
