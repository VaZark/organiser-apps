//! A [`Doc`](super::doc) back to Markdown. Text is escaped so that what was plain text parses
//! back as plain text: `parse(export(doc)) == doc`.

use crate::doc::{Align, Block, Definition, Footnote, Inline, Item};

/// Markdown for the blocks, blank-line separated, ending in one newline (or empty).
pub fn export(blocks: &[Block]) -> String {
    let body = blocks_md(blocks);
    if body.is_empty() {
        body
    } else {
        format!("{body}\n")
    }
}

fn blocks_md(blocks: &[Block]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (i, b) in blocks.iter().enumerate() {
        let md = block_md(b);
        // Two lists in a row would merge into one: an empty comment keeps them apart.
        if i > 0 && matches!(b, Block::List { .. }) && matches!(blocks[i - 1], Block::List { .. }) {
            parts.push("<!-- -->".into());
        }
        parts.push(md);
    }
    parts.join("\n\n")
}

fn block_md(b: &Block) -> String {
    match b {
        Block::Heading { level, id, content } => {
            let mut s = format!(
                "{} {}",
                "#".repeat(usize::from((*level).clamp(1, 6))),
                inlines(content)
            );
            if let Some(id) = id {
                s.push_str(&format!(" {{#{id}}}"));
            }
            s
        }
        Block::Paragraph(c) => paragraph(c),
        Block::Quote(inner) => prefix_lines(&blocks_md(inner), "> ", ">"),
        Block::List { start, items } => list(*start, items),
        Block::CodeBlock { lang, code } => {
            let code = code.strip_suffix('\n').unwrap_or(code);
            // A fence longer than any run of backticks inside it.
            let longest = code
                .lines()
                .map(|l| l.trim_start().chars().take_while(|c| *c == '`').count())
                .max()
                .unwrap_or(0);
            let fence = "`".repeat(longest.max(2) + 1);
            format!("{fence}{}\n{code}\n{fence}", lang.as_deref().unwrap_or(""))
        }
        Block::Rule => "---".into(),
        Block::Table { align, head, rows } => table(align, head, rows),
        Block::Definitions(defs) => definitions(defs),
        Block::Footnotes(notes) => footnotes(notes),
    }
}

/// A paragraph: lines that would read as another block (a list marker, a heading) are escaped.
fn paragraph(c: &[Inline]) -> String {
    inlines(c)
        .split('\n')
        .map(escape_line_start)
        .collect::<Vec<_>>()
        .join("\n")
}

fn escape_line_start(line: &str) -> String {
    let t = line.trim_start();
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    let numbered = digits > 0 && matches!(t[digits..].chars().next(), Some('.' | ')'));
    let marker = t.starts_with(['#', '>'])
        || t.starts_with("- ")
        || t.starts_with("+ ")
        || t == "-"
        || t == "+"
        || t.starts_with(['=', ':']) && t.chars().all(|c| matches!(c, '=' | ':' | '-'))
        || t.starts_with("---")
        || t.starts_with("```")
        || t.starts_with("~~~")
        || t.starts_with(": ");
    if numbered {
        let at = line.len() - t.len() + digits;
        format!("{}\\{}", &line[..at], &line[at..])
    } else if marker {
        let at = line.len() - t.len();
        format!("{}\\{}", &line[..at], &line[at..])
    } else {
        line.to_string()
    }
}

/// Prefix every line; blank lines get `blank` (so a quote stays one quote).
fn prefix_lines(s: &str, prefix: &str, blank: &str) -> String {
    s.lines()
        .map(|l| {
            if l.is_empty() {
                blank.to_string()
            } else {
                format!("{prefix}{l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn list(start: Option<u64>, items: &[Item]) -> String {
    let mut out: Vec<String> = Vec::new();
    for (n, item) in items.iter().enumerate() {
        let marker = match start {
            Some(s) => format!("{}.", s + n as u64),
            None => "-".to_string(),
        };
        let task = match item.checked {
            Some(true) => "[x] ",
            Some(false) => "[ ] ",
            None => "",
        };
        let lead = format!("{marker} {task}");
        let pad = " ".repeat(marker.len() + 1);
        // A nested list sits right under the text it belongs to, with no blank line.
        let mut body = String::new();
        for (i, b) in item.blocks.iter().enumerate() {
            if i > 0 {
                body.push_str(if matches!(b, Block::List { .. }) {
                    "\n"
                } else {
                    "\n\n"
                });
            }
            body.push_str(&block_md(b));
        }
        let mut lines = body.lines();
        let mut s = format!("{lead}{}", lines.next().unwrap_or(""));
        for l in lines {
            s.push('\n');
            if !l.is_empty() {
                s.push_str(&pad);
            }
            s.push_str(l);
        }
        out.push(s);
    }
    // Items made of single blocks stay tight; any item with blank lines inside makes it loose.
    let loose = items.iter().any(|i| {
        i.blocks.len() > 1
            && !i
                .blocks
                .iter()
                .skip(1)
                .all(|b| matches!(b, Block::List { .. }))
    });
    out.join(if loose { "\n\n" } else { "\n" })
}

fn table(align: &[Align], head: &[Vec<Inline>], rows: &[Vec<Vec<Inline>>]) -> String {
    let cols = rows
        .iter()
        .map(Vec::len)
        .chain([head.len(), align.len()])
        .max()
        .unwrap_or(0)
        .max(1);
    let line = |cells: &[Vec<Inline>]| {
        let cells: Vec<String> = (0..cols)
            .map(|c| {
                cells
                    .get(c)
                    .map(|x| inlines(x).replace('|', "\\|"))
                    .unwrap_or_default()
            })
            .collect();
        format!("| {} |", cells.join(" | "))
    };
    let rule: Vec<&str> = (0..cols)
        .map(|c| match align.get(c).copied().unwrap_or_default() {
            Align::Left => "---",
            Align::Center => ":---:",
            Align::Right => "---:",
        })
        .collect();
    let mut out = vec![line(head), format!("| {} |", rule.join(" | "))];
    out.extend(rows.iter().map(|r| line(r)));
    out.join("\n")
}

fn definitions(defs: &[Definition]) -> String {
    defs.iter()
        .map(|d| {
            let mut s = inlines(&d.term);
            for x in &d.details {
                s.push_str(&format!("\n: {}", inlines(x)));
            }
            s
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn footnotes(notes: &[Footnote]) -> String {
    notes
        .iter()
        .map(|n| {
            let body = blocks_md(&n.blocks);
            let mut lines = body.lines();
            let mut s = format!("[^{}]: {}", n.label, lines.next().unwrap_or(""));
            for l in lines {
                s.push('\n');
                if !l.is_empty() {
                    s.push_str("    ");
                }
                s.push_str(l);
            }
            s
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

// ---- inlines -------------------------------------------------------------------------------

fn inlines(c: &[Inline]) -> String {
    c.iter().map(inline).collect()
}

fn inline(i: &Inline) -> String {
    match i {
        Inline::Text(t) => escape(t),
        Inline::Code(c) => code_span(c),
        Inline::Emphasis(c) => wrap("*", c),
        Inline::Strong(c) => wrap("**", c),
        Inline::Strike(c) => wrap("~~", c),
        Inline::Highlight(c) => wrap("==", c),
        Inline::Subscript(c) => wrap("~", c),
        Inline::Superscript(c) => wrap("^", c),
        Inline::Link { content, url } => format!("[{}]({})", inlines(content), dest(url)),
        Inline::Image { alt, url } => format!("![{}]({})", escape(alt), dest(url)),
        Inline::FootnoteRef(l) => format!("[^{l}]"),
        Inline::LineBreak => "\\\n".into(),
    }
}

/// `inner` between `mark`s. Markers cannot touch a space inside, so leading and trailing
/// spaces move outside them.
fn wrap(mark: &str, c: &[Inline]) -> String {
    let inner = inlines(c);
    let t = inner.trim();
    if t.is_empty() {
        return inner;
    }
    let lead = &inner[..inner.len() - inner.trim_start().len()];
    let trail = &inner[inner.trim_end().len()..];
    format!("{lead}{mark}{t}{mark}{trail}")
}

fn code_span(c: &str) -> String {
    let longest = c.split(|ch| ch != '`').map(str::len).max().unwrap_or(0);
    let ticks = "`".repeat(longest + 1);
    let pad = if c.starts_with('`')
        || c.ends_with('`')
        || c.starts_with(' ') && c.ends_with(' ') && c.trim() != ""
    {
        " "
    } else {
        ""
    };
    format!("{ticks}{pad}{c}{pad}{ticks}")
}

/// A link target; one with spaces or parentheses goes in angle brackets.
fn dest(url: &str) -> String {
    if url.contains([' ', '(', ')']) {
        format!("<{url}>")
    } else {
        url.to_string()
    }
}

/// Characters that would start formatting, so plain text keeps meaning what it said.
fn escape(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    let mut chars = t.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' | '*' | '_' | '`' | '[' | ']' | '<' | '>' | '~' | '^' | '|' | '&' => {
                out.push('\\');
                out.push(c);
            }
            '=' if chars.peek() == Some(&'=') => out.push_str("\\="),
            ':' if emoji_follows(&chars) => out.push_str("\\:"),
            _ => out.push(c),
        }
    }
    out
}

/// Whether a `:` followed by these characters could read as an emoji shortcode.
fn emoji_follows(rest: &std::iter::Peekable<std::str::Chars>) -> bool {
    let word: String = rest
        .clone()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '-'))
        .collect();
    !word.is_empty() && rest.clone().nth(word.chars().count()) == Some(':')
}
