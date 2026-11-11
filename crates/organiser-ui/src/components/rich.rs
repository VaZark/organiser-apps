//! The organiser apps' Markdown document to the one `iced-adaptive-design` draws. A
//! field-for-field mapping: the Markdown rules are in `organiser-markdown`, the drawing in iad.

use crate::iad::primitives::rich_text as rt;
use organiser_markdown::doc as n;

pub fn blocks(doc: &[n::Block]) -> Vec<rt::Block> {
    doc.iter().map(block).collect()
}

fn inlines(c: &[n::Inline]) -> Vec<rt::Inline> {
    c.iter().map(inline).collect()
}

fn inline(i: &n::Inline) -> rt::Inline {
    match i {
        n::Inline::Text(t) => rt::Inline::Text(t.clone()),
        n::Inline::Code(t) => rt::Inline::Code(t.clone()),
        n::Inline::Emphasis(c) => rt::Inline::Emphasis(inlines(c)),
        n::Inline::Strong(c) => rt::Inline::Strong(inlines(c)),
        n::Inline::Strike(c) => rt::Inline::Strike(inlines(c)),
        n::Inline::Highlight(c) => rt::Inline::Highlight(inlines(c)),
        n::Inline::Subscript(c) => rt::Inline::Subscript(inlines(c)),
        n::Inline::Superscript(c) => rt::Inline::Superscript(inlines(c)),
        n::Inline::Link { content, url } => rt::Inline::Link {
            content: inlines(content),
            url: url.clone(),
        },
        n::Inline::Image { alt, url } => rt::Inline::Image {
            alt: alt.clone(),
            url: url.clone(),
        },
        n::Inline::FootnoteRef(l) => rt::Inline::FootnoteRef(l.clone()),
        n::Inline::LineBreak => rt::Inline::LineBreak,
    }
}

fn block(b: &n::Block) -> rt::Block {
    match b {
        n::Block::Heading { level, id, content } => rt::Block::Heading {
            level: *level,
            id: id.clone(),
            content: inlines(content),
        },
        n::Block::Paragraph(c) => rt::Block::Paragraph(inlines(c)),
        n::Block::Quote(inner) => rt::Block::Quote(blocks(inner)),
        n::Block::List { start, items } => rt::Block::List {
            start: *start,
            items: items
                .iter()
                .map(|i| rt::Item {
                    checked: i.checked,
                    blocks: blocks(&i.blocks),
                })
                .collect(),
        },
        n::Block::CodeBlock { lang, code } => rt::Block::CodeBlock {
            lang: lang.clone(),
            code: code.clone(),
        },
        n::Block::Rule => rt::Block::Rule,
        n::Block::Table { align, head, rows } => rt::Block::Table {
            align: align
                .iter()
                .map(|a| match a {
                    n::Align::Left => rt::Align::Left,
                    n::Align::Center => rt::Align::Center,
                    n::Align::Right => rt::Align::Right,
                })
                .collect(),
            head: head.iter().map(|c| inlines(c)).collect(),
            rows: rows
                .iter()
                .map(|r| r.iter().map(|c| inlines(c)).collect())
                .collect(),
        },
        n::Block::Definitions(defs) => rt::Block::Definitions(
            defs.iter()
                .map(|d| rt::Definition {
                    term: inlines(&d.term),
                    details: d.details.iter().map(|x| inlines(x)).collect(),
                })
                .collect(),
        ),
        n::Block::Footnotes(notes) => rt::Block::Footnotes(
            notes
                .iter()
                .map(|f| rt::Footnote {
                    label: f.label.clone(),
                    blocks: blocks(&f.blocks),
                })
                .collect(),
        ),
    }
}

/// A note body, parsed and ready to draw.
pub fn render(markdown: &str) -> Vec<rt::Block> {
    blocks(&organiser_markdown::parse::parse(markdown))
}
