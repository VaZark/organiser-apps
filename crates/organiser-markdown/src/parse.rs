//! Markdown to a [`Doc`](super::doc): the basic syntax and the extended one of the Markdown
//! Guide's cheat sheet (tables, fenced code, footnotes, heading ids, definition lists,
//! strike-through, task lists, emoji shortcodes, highlight, sub- and superscript).
//!
//! Parsing never fails: text that is not valid Markdown is just text.

use std::iter::Peekable;

use pulldown_cmark::{
    Alignment, CodeBlockKind, CowStr, Event, HeadingLevel, OffsetIter, Options, Parser, Tag, TagEnd,
};

use crate::doc::{Align, Block, Definition, Footnote, Inline, Item};

/// The characters our own inline rules (`==`, `~`, `^`, `:emoji:`) act on. When the author
/// escaped one (`\~`), it is swapped for a private-use character while the text is read, so
/// no rule fires on it, and swapped back afterwards.
const PROTECTED: [(char, char); 4] = [
    ('~', '\u{E000}'),
    ('^', '\u{E001}'),
    ('=', '\u{E002}'),
    (':', '\u{E003}'),
];

fn restore(s: String) -> String {
    if !s.chars().any(|c| ('\u{E000}'..='\u{E003}').contains(&c)) {
        return s;
    }
    s.chars()
        .map(|c| {
            PROTECTED
                .iter()
                .find(|(_, p)| *p == c)
                .map_or(c, |(o, _)| *o)
        })
        .collect()
}

/// The parser's events, with escaped marker characters protected (see [`PROTECTED`]).
struct Events<'a> {
    it: Peekable<OffsetIter<'a>>,
    src: &'a str,
}

impl<'a> Events<'a> {
    fn new(md: &'a str) -> Self {
        Self {
            it: Parser::new_ext(md, options()).into_offset_iter().peekable(),
            src: md,
        }
    }

    fn peek(&mut self) -> Option<&Event<'a>> {
        self.it.peek().map(|(e, _)| e)
    }
}

impl<'a> Iterator for Events<'a> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Event<'a>> {
        let (e, range) = self.it.next()?;
        if let Event::Text(t) = &e
            && let Some(c) = t.chars().next()
            && let Some((_, p)) = PROTECTED.iter().find(|(o, _)| *o == c)
            // The source has an (unescaped) backslash right before it: the author escaped it.
            // The parser's range for the text starts just after the backslash.
            && self.src.get(..range.start).is_some_and(|before| {
                before.bytes().rev().take_while(|b| *b == b'\\').count() % 2 == 1
            })
        {
            let mut text = p.to_string();
            text.push_str(&t[c.len_utf8()..]);
            return Some(Event::Text(CowStr::from(text)));
        }
        Some(e)
    }
}

fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_HEADING_ATTRIBUTES
        | Options::ENABLE_DEFINITION_LIST
        | Options::ENABLE_SUPERSCRIPT
        | Options::ENABLE_SUBSCRIPT
}

pub fn parse(md: &str) -> Vec<Block> {
    let mut it = Events::new(md);
    let mut blocks = Vec::new();
    let mut notes = Vec::new();
    read_blocks(&mut it, &mut blocks, &mut notes);
    if !notes.is_empty() {
        blocks.push(Block::Footnotes(notes));
    }
    blocks
}

/// Emoji shortcodes the cheat sheet's `:joy:` form covers: the common ones.
const EMOJI: &[(&str, &str)] = &[
    ("smile", "😄"),
    ("grin", "😁"),
    ("joy", "😂"),
    ("wink", "😉"),
    ("heart", "❤️"),
    ("thumbsup", "👍"),
    ("+1", "👍"),
    ("thumbsdown", "👎"),
    ("tada", "🎉"),
    ("fire", "🔥"),
    ("star", "⭐"),
    ("sparkles", "✨"),
    ("rocket", "🚀"),
    ("warning", "⚠️"),
    ("white_check_mark", "✅"),
    ("x", "❌"),
    ("question", "❓"),
    ("bulb", "💡"),
    ("memo", "📝"),
    ("pushpin", "📌"),
    ("book", "📖"),
    ("calendar", "📅"),
    ("clock", "🕒"),
    ("eyes", "👀"),
    ("pray", "🙏"),
    ("clap", "👏"),
    ("sunny", "☀️"),
    ("coffee", "☕"),
];

fn is_code_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '+' || c == '-'
}

/// Plain text with `:shortcode:` emoji replaced and `==marked==` text split out.
fn text_runs(s: &str) -> Vec<Inline> {
    let mut out = Vec::new();
    let mut rest = s;
    let mut plain = String::new();
    let flush = |plain: &mut String, out: &mut Vec<Inline>| {
        if !plain.is_empty() {
            out.push(Inline::Text(std::mem::take(plain)));
        }
    };
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("==")
            && let Some(end) = after.find("==")
            && end > 0
            && !after[..end].starts_with(char::is_whitespace)
        {
            flush(&mut plain, &mut out);
            out.push(Inline::Highlight(text_runs(&after[..end])));
            rest = &after[end + 2..];
            continue;
        }
        // `H~2~O` and `X^2^`: a mark, text without spaces, the same mark.
        if let Some((tag, mark)) = ["~", "^"]
            .into_iter()
            .find(|m| rest.starts_with(m))
            .map(|m| (m, m))
            && !rest.starts_with("~~")
        {
            let after = &rest[mark.len()..];
            if let Some(end) = after.find(tag)
                && end > 0
                && !after[..end].contains(char::is_whitespace)
                && !after[..end].contains(['~', '^'])
            {
                flush(&mut plain, &mut out);
                let inner = text_runs(&after[..end]);
                out.push(if tag == "~" {
                    Inline::Subscript(inner)
                } else {
                    Inline::Superscript(inner)
                });
                rest = &after[end + 1..];
                continue;
            }
        }
        if let Some(after) = rest.strip_prefix(':')
            && let Some(end) = after.find(':')
            && end > 0
            && after[..end].chars().all(is_code_char)
            && let Some((_, e)) = EMOJI.iter().find(|(n, _)| *n == &after[..end])
        {
            plain.push_str(e);
            rest = &after[end + 1..];
            continue;
        }
        let c = rest.chars().next().expect("not empty");
        plain.push(c);
        rest = &rest[c.len_utf8()..];
    }
    flush(&mut plain, &mut out);
    out
}

/// Whether an event is part of a line of text (so a loose run of them is a paragraph).
fn is_inline(e: &Event) -> bool {
    match e {
        Event::Text(_)
        | Event::Code(_)
        | Event::SoftBreak
        | Event::HardBreak
        | Event::FootnoteReference(_)
        | Event::InlineHtml(_)
        | Event::InlineMath(_)
        | Event::DisplayMath(_) => true,
        Event::Start(t) => matches!(
            t,
            Tag::Emphasis
                | Tag::Strong
                | Tag::Strikethrough
                | Tag::Superscript
                | Tag::Subscript
                | Tag::Link { .. }
                | Tag::Image { .. }
        ),
        _ => false,
    }
}

/// Inline content up to the `End` that closes the surrounding tag (consumed), or, with
/// `until_end` false, up to the first event that is not inline (left alone).
fn read_inlines(it: &mut Events, until_end: bool) -> Vec<Inline> {
    let mut out = Vec::new();
    loop {
        match it.peek() {
            None => break,
            Some(Event::End(_)) => {
                if until_end {
                    it.next();
                }
                break;
            }
            Some(e) if !until_end && !is_inline(e) => break,
            Some(_) => {}
        }
        let Some(e) = it.next() else { break };
        match e {
            Event::Text(t) => {
                // The parser splits text at escapes and markers; our rules need it whole.
                let mut whole = t.into_string();
                while let Some(Event::Text(_)) = it.peek() {
                    if let Some(Event::Text(more)) = it.next() {
                        whole.push_str(&more);
                    }
                }
                for run in text_runs(&whole) {
                    out.push(restore_inline(run));
                }
            }
            Event::Code(c) => out.push(Inline::Code(c.into_string())),
            Event::SoftBreak => out.push(Inline::Text(" ".into())),
            Event::HardBreak => out.push(Inline::LineBreak),
            Event::FootnoteReference(l) => out.push(Inline::FootnoteRef(l.into_string())),
            Event::InlineHtml(h) | Event::InlineMath(h) | Event::DisplayMath(h) => {
                out.push(Inline::Text(h.into_string()));
            }
            Event::Start(tag) => {
                let inner = read_inlines(it, true);
                out.push(match tag {
                    Tag::Emphasis => Inline::Emphasis(inner),
                    Tag::Strong => Inline::Strong(inner),
                    Tag::Strikethrough => Inline::Strike(inner),
                    Tag::Superscript => Inline::Superscript(inner),
                    Tag::Subscript => Inline::Subscript(inner),
                    Tag::Link { dest_url, .. } => Inline::Link {
                        content: inner,
                        url: dest_url.into_string(),
                    },
                    Tag::Image { dest_url, .. } => Inline::Image {
                        alt: plain(&inner),
                        url: dest_url.into_string(),
                    },
                    _ => continue,
                });
            }
            _ => {}
        }
    }
    out
}

fn restore_inline(i: Inline) -> Inline {
    match i {
        Inline::Text(t) => Inline::Text(restore(t)),
        Inline::Highlight(c) => Inline::Highlight(c.into_iter().map(restore_inline).collect()),
        Inline::Subscript(c) => Inline::Subscript(c.into_iter().map(restore_inline).collect()),
        Inline::Superscript(c) => Inline::Superscript(c.into_iter().map(restore_inline).collect()),
        other => other,
    }
}

fn plain(inlines: &[Inline]) -> String {
    inlines
        .iter()
        .map(|i| match i {
            Inline::Text(t) | Inline::Code(t) => t.clone(),
            Inline::Emphasis(c)
            | Inline::Strong(c)
            | Inline::Strike(c)
            | Inline::Highlight(c)
            | Inline::Subscript(c)
            | Inline::Superscript(c)
            | Inline::Link { content: c, .. } => plain(c),
            Inline::Image { alt, .. } => alt.clone(),
            Inline::FootnoteRef(_) => String::new(),
            Inline::LineBreak => "\n".into(),
        })
        .collect()
}

/// An HTML comment draws nothing (the exporter uses one to keep two lists apart).
fn is_comment(h: &str) -> bool {
    let t = h.trim();
    t.starts_with("<!--") && t.ends_with("-->")
}

fn level(l: HeadingLevel) -> u8 {
    match l {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn align(a: Alignment) -> Align {
    match a {
        Alignment::None | Alignment::Left => Align::Left,
        Alignment::Center => Align::Center,
        Alignment::Right => Align::Right,
    }
}

/// Blocks up to the `End` that closes the container (consumed), or the end of input.
/// Footnote definitions found on the way go to `notes`.
fn read_blocks(it: &mut Events, out: &mut Vec<Block>, notes: &mut Vec<Footnote>) {
    while let Some(e) = it.peek() {
        if matches!(e, Event::End(_)) {
            it.next();
            return;
        }
        if is_inline(e) {
            let run = read_inlines(it, false);
            if !run.is_empty() {
                out.push(Block::Paragraph(run));
            }
            continue;
        }
        let Some(e) = it.next() else { return };
        match e {
            Event::Rule => out.push(Block::Rule),
            Event::Html(h) if is_comment(&h) => {}
            Event::Html(h) => out.push(Block::Paragraph(vec![Inline::Text(
                h.trim_end().to_string(),
            )])),
            Event::Start(tag) => match tag {
                Tag::Paragraph => out.push(Block::Paragraph(read_inlines(it, true))),
                Tag::Heading { level: l, id, .. } => out.push(Block::Heading {
                    level: level(l),
                    id: id.map(|i| i.into_string()),
                    content: read_inlines(it, true),
                }),
                Tag::BlockQuote(_) => {
                    let mut inner = Vec::new();
                    read_blocks(it, &mut inner, notes);
                    out.push(Block::Quote(inner));
                }
                Tag::CodeBlock(kind) => {
                    let mut code = String::new();
                    for e in it.by_ref() {
                        match e {
                            Event::Text(t) => code.push_str(&t),
                            Event::End(_) => break,
                            _ => {}
                        }
                    }
                    let lang = match kind {
                        CodeBlockKind::Fenced(l) => {
                            let l = l.split_whitespace().next().unwrap_or("").to_string();
                            (!l.is_empty()).then_some(l)
                        }
                        CodeBlockKind::Indented => None,
                    };
                    out.push(Block::CodeBlock { lang, code });
                }
                Tag::List(start) => out.push(read_list(it, start, notes)),
                Tag::FootnoteDefinition(label) => {
                    let mut inner = Vec::new();
                    read_blocks(it, &mut inner, notes);
                    notes.push(Footnote {
                        label: label.into_string(),
                        blocks: inner,
                    });
                }
                Tag::Table(aligns) => out.push(read_table(it, &aligns)),
                Tag::DefinitionList => out.push(read_definitions(it)),
                Tag::HtmlBlock => {
                    let mut html = String::new();
                    for e in it.by_ref() {
                        match e {
                            Event::Html(h) | Event::Text(h) => html.push_str(&h),
                            Event::End(_) => break,
                            _ => {}
                        }
                    }
                    if !html.trim().is_empty() && !is_comment(&html) {
                        out.push(Block::Paragraph(vec![Inline::Text(html.trim_end().into())]));
                    }
                }
                // A tag we do not draw (metadata, say): keep what is inside it.
                _ => read_blocks(it, out, notes),
            },
            _ => {}
        }
    }
}

fn read_list(it: &mut Events, start: Option<u64>, notes: &mut Vec<Footnote>) -> Block {
    let mut items = Vec::new();
    while let Some(e) = it.next() {
        match e {
            Event::Start(Tag::Item) => {
                let checked = match it.peek() {
                    Some(Event::TaskListMarker(c)) => {
                        let c = *c;
                        it.next();
                        Some(c)
                    }
                    _ => None,
                };
                let mut blocks = Vec::new();
                read_blocks(it, &mut blocks, notes);
                items.push(Item { checked, blocks });
            }
            Event::End(_) => break,
            _ => {}
        }
    }
    Block::List { start, items }
}

fn read_table(it: &mut Events, aligns: &[Alignment]) -> Block {
    let mut head = Vec::new();
    let mut rows = Vec::new();
    let mut current: Option<Vec<Vec<Inline>>> = None;
    while let Some(e) = it.next() {
        match e {
            Event::Start(Tag::TableHead | Tag::TableRow) => current = Some(Vec::new()),
            Event::Start(Tag::TableCell) => {
                let cell = read_inlines(it, true);
                if let Some(c) = &mut current {
                    c.push(cell);
                }
            }
            Event::End(TagEnd::TableHead) => head = current.take().unwrap_or_default(),
            Event::End(TagEnd::TableRow) => rows.extend(current.take()),
            Event::End(TagEnd::Table) => break,
            _ => {}
        }
    }
    Block::Table {
        align: aligns.iter().copied().map(align).collect(),
        head,
        rows,
    }
}

fn read_definitions(it: &mut Events) -> Block {
    let mut defs: Vec<Definition> = Vec::new();
    while let Some(e) = it.next() {
        match e {
            Event::Start(Tag::DefinitionListTitle) => defs.push(Definition {
                term: read_inlines(it, true),
                details: Vec::new(),
            }),
            Event::Start(Tag::DefinitionListDefinition) => {
                let d = read_inlines(it, true);
                if let Some(last) = defs.last_mut() {
                    last.details.push(d);
                }
            }
            Event::End(TagEnd::DefinitionList) => break,
            _ => {}
        }
    }
    Block::Definitions(defs)
}
