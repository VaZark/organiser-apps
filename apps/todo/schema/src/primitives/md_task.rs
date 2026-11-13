//! A whole task as a Markdown file: front matter for what the task has, the title as a first-level
//! heading, then its notes. Where the file is says which epic it is in and whose subtask it is, so
//! neither is in it.

use chrono::NaiveDate;
use organiser_vault::front_matter::{self, FrontMatter};

use super::priority::Priority;
use super::task::Task;

/// What a vault file says about its task (see [`parse_file`]).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FileTask {
    /// The task it is, when the file says (an `id:` in its front matter).
    pub id: Option<i64>,
    pub title: String,
    /// The task's notes.
    pub notes: String,
    pub tags: Vec<String>,
    pub pinned: bool,
    pub priority: Option<Priority>,
    pub start: Option<NaiveDate>,
    pub due: Option<NaiveDate>,
    /// The name of its status, as the person has named it.
    pub status: Option<String>,
    /// The name of its section in its epic.
    pub section: Option<String>,
    pub archived: bool,
    pub created: Option<i64>,
    /// The block as read, so a key this app does not know is kept when the file is rewritten.
    pub front: Option<FrontMatter>,
}

/// The title and body of Markdown: a first-level heading on the first line is the title (and is
/// not repeated in the body); otherwise `fallback` (a file name) is.
fn title_and_body(md: &str, fallback: &str) -> (String, String) {
    let md = md.strip_prefix('\u{feff}').unwrap_or(md);
    let (first, rest) = md.split_once('\n').unwrap_or((md, ""));
    match first.strip_prefix("# ").map(str::trim) {
        Some(t) if !t.is_empty() => (t.to_string(), rest.trim_matches('\n').to_string()),
        _ => (fallback.to_string(), md.trim_matches('\n').to_string()),
    }
}

/// A task from a Markdown file, with or without front matter.
pub fn parse_file(text: &str, fallback: &str) -> FileTask {
    let (front, rest) = front_matter::split(text);
    let (title, notes) = title_and_body(rest, fallback);
    let mut t = FileTask {
        title,
        notes,
        ..FileTask::default()
    };
    if let Some(fm) = &front {
        t.id = fm.get_i64("id");
        t.tags = fm.list("tags");
        t.pinned = fm.get_bool("pinned").unwrap_or(false);
        t.priority = fm.get("priority").and_then(|p| Priority::parse(&p.to_lowercase()));
        t.start = fm.get("start").and_then(|d| d.parse().ok());
        t.due = fm.get("due").and_then(|d| d.parse().ok());
        t.status = fm.get("status");
        t.section = fm.get("section");
        t.archived = fm.get_bool("archived").unwrap_or(false);
        t.created = fm.get("created").and_then(|c| front_matter::parse_iso(&c));
    }
    t.front = front;
    t
}

/// The file for `task`: front matter, the title as a heading, then the notes. `status` and
/// `section` are the names of the task's own; `previous` is the block the file had, so keys that
/// are not this app's are kept.
pub fn render_file(
    task: &Task,
    status: Option<&str>,
    section: Option<&str>,
    previous: Option<&FrontMatter>,
) -> String {
    let mut fm = previous.cloned().unwrap_or_default();
    fm.set_raw("id", &task.id.to_string());
    match status {
        Some(s) => fm.set("status", s),
        None => fm.remove("status"),
    }
    fm.set("priority", task.priority.as_str());
    match task.start {
        Some(d) => fm.set_raw("start", &d.to_string()),
        None => fm.remove("start"),
    }
    match task.due {
        Some(d) => fm.set_raw("due", &d.to_string()),
        None => fm.remove("due"),
    }
    if task.tags.is_empty() {
        fm.remove("tags");
    } else {
        fm.set_list("tags", &task.tags);
    }
    for (key, on) in [("pinned", task.pinned), ("archived", task.archived)] {
        if on {
            fm.set_raw(key, "true");
        } else {
            fm.remove(key);
        }
    }
    match section {
        Some(s) => fm.set("section", s),
        None => fm.remove("section"),
    }
    fm.set_raw("created", &front_matter::iso(task.created));
    let title = task.title.replace('\n', " ");
    let notes = task.notes.trim_matches('\n');
    if notes.is_empty() {
        format!("{}# {title}\n", fm.render())
    } else {
        format!("{}# {title}\n\n{notes}\n", fm.render())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(title: &str, notes: &str) -> Task {
        Task {
            id: 7,
            parent_id: None,
            epic: None,
            section: None,
            title: title.into(),
            notes: notes.into(),
            status: None,
            priority: Priority::Medium,
            start: None,
            due: None,
            tags: vec![],
            position: 0,
            archived: false,
            sticky: false,
            pinned: false,
            created: 1_759_305_600,
            updated: 0,
        }
    }

    #[test]
    fn a_file_with_front_matter_round_trips() {
        let mut t = task("Book flights", "Compare **prices**.\n\n- [ ] Lisbon");
        t.priority = Priority::High;
        t.start = NaiveDate::from_ymd_opt(2026, 11, 10);
        t.due = NaiveDate::from_ymd_opt(2026, 11, 20);
        t.tags = vec!["travel".into(), "a, b".into()];
        t.pinned = true;
        let text = render_file(&t, Some("In progress"), Some("Booking"), None);
        assert!(text.starts_with("---\nid: 7\n"));
        let back = parse_file(&text, "x");
        assert_eq!(back.id, Some(7));
        assert_eq!(back.title, "Book flights");
        assert_eq!(back.notes, "Compare **prices**.\n\n- [ ] Lisbon");
        assert_eq!(back.status.as_deref(), Some("In progress"));
        assert_eq!(back.priority, Some(Priority::High));
        assert_eq!((back.start, back.due), (t.start, t.due));
        assert_eq!(back.tags, ["travel", "a, b"]);
        assert!(back.pinned && !back.archived);
        assert_eq!(back.section.as_deref(), Some("Booking"));
        assert_eq!(back.created, Some(1_759_305_600));
        // Written again from what was read, it is the same file.
        assert_eq!(render_file(&t, Some("In progress"), Some("Booking"), back.front.as_ref()), text);
    }

    #[test]
    fn a_plain_file_is_a_task_with_its_name_as_title() {
        let f = parse_file("just words\n", "Buy milk");
        assert_eq!((f.title.as_str(), f.notes.as_str()), ("Buy milk", "just words"));
        assert!(f.id.is_none() && f.front.is_none() && f.status.is_none());
        let f = parse_file("# Title\n\nbody\n", "stem");
        assert_eq!((f.title.as_str(), f.notes.as_str()), ("Title", "body"));
    }

    #[test]
    fn keys_of_other_tools_are_kept_and_cleared_ones_leave() {
        let f = parse_file("---\nid: 1\npinned: true\ncssclass: wide\ndue: 2026-01-01\n---\n# T\n", "s");
        let mut t = task("T", "");
        t.id = 1;
        let text = render_file(&t, None, None, f.front.as_ref());
        assert!(text.contains("cssclass: wide"));
        assert!(!text.contains("pinned") && !text.contains("due:"));
    }

    #[test]
    fn a_bad_priority_or_date_is_ignored() {
        let f = parse_file("---\npriority: urgent!\ndue: someday\n---\n# T\n", "s");
        assert_eq!((f.priority, f.due), (None, None));
    }
}
