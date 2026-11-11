//! The Markdown Guide's cheat sheet, element by element: each parses to the right shape, and a
//! whole document survives a round trip through the exporter.

use organiser_markdown::doc::{Align, Block, Definition, Footnote, Inline, Item};
use organiser_markdown::export::export;
use organiser_markdown::parse::parse;

fn t(s: &str) -> Vec<Inline> {
    vec![Inline::Text(s.into())]
}

fn first(md: &str) -> Block {
    parse(md).into_iter().next().expect("one block")
}

#[test]
fn headings_have_levels_and_optional_ids() {
    assert_eq!(
        first("# Title"),
        Block::Heading {
            level: 1,
            id: None,
            content: t("Title")
        }
    );
    assert_eq!(
        first("### Third"),
        Block::Heading {
            level: 3,
            id: None,
            content: t("Third")
        }
    );
    assert_eq!(
        first("### My Great Heading {#custom-id}"),
        Block::Heading {
            level: 3,
            id: Some("custom-id".into()),
            content: t("My Great Heading")
        }
    );
}

#[test]
fn emphasis_strike_highlight_and_scripts() {
    let Block::Paragraph(p) = first("**bold** *it* ~~gone~~ ==marked== H~2~O X^2^") else {
        panic!()
    };
    assert!(p.contains(&Inline::Strong(t("bold"))));
    assert!(p.contains(&Inline::Emphasis(t("it"))));
    assert!(p.contains(&Inline::Strike(t("gone"))));
    assert!(p.contains(&Inline::Highlight(t("marked"))));
    assert!(p.contains(&Inline::Subscript(t("2"))));
    assert!(p.contains(&Inline::Superscript(t("2"))));
}

#[test]
fn blockquotes_nest() {
    let Block::Quote(inner) = first("> outer\n>\n> > inner") else {
        panic!()
    };
    assert_eq!(inner[0], Block::Paragraph(t("outer")));
    assert!(matches!(inner[1], Block::Quote(_)));
}

#[test]
fn ordered_and_unordered_lists_with_nesting() {
    let Block::List { start, items } = first("1. First\n2. Second\n    - nested\n3. Third") else {
        panic!()
    };
    assert_eq!(start, Some(1));
    assert_eq!(items.len(), 3);
    assert!(matches!(
        items[1].blocks[1],
        Block::List { start: None, .. }
    ));
    let Block::List { start, .. } = first("- a\n- b") else {
        panic!()
    };
    assert_eq!(start, None);
}

#[test]
fn task_lists_carry_their_state() {
    let Block::List { items, .. } = first("- [x] done\n- [ ] todo\n- plain") else {
        panic!()
    };
    let state: Vec<_> = items.iter().map(|i| i.checked).collect();
    assert_eq!(state, [Some(true), Some(false), None]);
    assert_eq!(items[0].blocks, vec![Block::Paragraph(t("done"))]);
}

#[test]
fn code_inline_and_fenced() {
    let Block::Paragraph(p) = first("use `code` here") else {
        panic!()
    };
    assert!(p.contains(&Inline::Code("code".into())));
    assert_eq!(
        first("```json\n{\"a\": 1}\n```"),
        Block::CodeBlock {
            lang: Some("json".into()),
            code: "{\"a\": 1}\n".into()
        }
    );
    assert_eq!(
        first("```\nplain\n```"),
        Block::CodeBlock {
            lang: None,
            code: "plain\n".into()
        }
    );
}

#[test]
fn rules_links_and_images() {
    assert_eq!(first("---"), Block::Rule);
    let Block::Paragraph(p) = first("[Duck](https://duck.com) ![alt text](img.jpg)") else {
        panic!()
    };
    assert!(p.contains(&Inline::Link {
        content: t("Duck"),
        url: "https://duck.com".into()
    }));
    assert!(p.contains(&Inline::Image {
        alt: "alt text".into(),
        url: "img.jpg".into()
    }));
}

#[test]
fn tables_keep_alignment_head_and_rows() {
    let Block::Table { align, head, rows } =
        first("| A | B | C |\n| :-- | :-: | --: |\n| 1 | 2 | 3 |\n| 4 | 5 | 6 |")
    else {
        panic!()
    };
    assert_eq!(align, [Align::Left, Align::Center, Align::Right]);
    assert_eq!(head, vec![t("A"), t("B"), t("C")]);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1][2], t("6"));
}

#[test]
fn footnotes_collect_at_the_end() {
    let doc = parse("Here is a note.[^1]\n\n[^1]: And the note.");
    let Block::Paragraph(p) = &doc[0] else {
        panic!()
    };
    assert!(p.contains(&Inline::FootnoteRef("1".into())));
    assert_eq!(
        doc.last(),
        Some(&Block::Footnotes(vec![Footnote {
            label: "1".into(),
            blocks: vec![Block::Paragraph(t("And the note."))]
        }]))
    );
}

#[test]
fn definition_lists_pair_terms_with_details() {
    assert_eq!(
        first(
            "First Term\n: This is the definition of the first term.\n\nSecond Term\n: One.\n: Two."
        ),
        Block::Definitions(vec![
            Definition {
                term: t("First Term"),
                details: vec![t("This is the definition of the first term.")]
            },
            Definition {
                term: t("Second Term"),
                details: vec![t("One."), t("Two.")]
            },
        ])
    );
}

#[test]
fn emoji_shortcodes_become_emoji_and_unknown_ones_stay() {
    let Block::Paragraph(p) = first("That is funny :joy: and :nonsense:") else {
        panic!()
    };
    assert_eq!(p, t("That is funny 😂 and :nonsense:"));
}

const CHEAT_SHEET: &str = "# Heading 1\n\n## Heading 2\n\n### Heading 3 {#custom-id}\n\nI just love **bold text**. Italicized text is the *cat's meow*. This is ***really important***.\n\n> Dorothy followed her down.\n>\n> > The quarry.\n\n1. First item\n2. Second item\n    - Indented\n    - Again\n3. Third item\n\n- First\n- Second\n\nAt the command prompt, type `nano`.\n\n---\n\nMy favorite search engine is [Duck Duck Go](https://duckduckgo.com \"Search\").\n\n![The San Juan Mountains](img/mountains.jpg)\n\n| Syntax | Description |\n| ----------- | ------:|\n| Header | Title |\n| Paragraph | Text |\n\n```json\n{\n  \"firstName\": \"John\"\n}\n```\n\nHere's a sentence with a footnote.[^1]\n\nFirst Term\n: This is the definition of the first term.\n\n~~The world is flat.~~ We now know that the world is round.\n\n- [x] Write the press release\n- [ ] Update the website\n\nI need to highlight these ==very important words==.\n\nH~2~O and X^2^\n\nGone camping! :tent: :joy:\n\n[^1]: This is the footnote.\n";

#[test]
fn a_whole_cheat_sheet_survives_a_round_trip() {
    let doc = parse(CHEAT_SHEET);
    let md = export(&doc);
    assert_eq!(parse(&md), doc, "exported:\n{md}");
}

#[test]
fn export_is_stable_after_one_pass() {
    let once = export(&parse(CHEAT_SHEET));
    assert_eq!(export(&parse(&once)), once);
}

#[test]
fn export_writes_the_cheat_sheet_syntax() {
    let md = export(&parse(CHEAT_SHEET));
    for want in [
        "### Heading 3 {#custom-id}",
        "**bold text**",
        "- [x] Write the press release",
        "- [ ] Update the website",
        "==very important words==",
        "H~2~O",
        "X^2^",
        "~~The world is flat.~~",
        "```json\n{\n  \"firstName\": \"John\"\n}\n```",
        "| Syntax | Description |",
        "---:",
        "[^1]: This is the footnote.",
        "First Term\n: This is the definition of the first term.",
        "> > The quarry.",
        "---\n",
    ] {
        assert!(md.contains(want), "missing {want:?} in:\n{md}");
    }
}

#[test]
fn text_that_looks_like_syntax_stays_text() {
    for src in [
        "2 * 3 * 4 and snake_case_name",
        "a < b > c & d",
        "price: 5 | 6 and a [bracket]",
        "# not a heading",
        "1. not a list",
        "- not a list either",
        "x ~ y ^ z == w",
        "keep :joy: out of it",
    ] {
        let doc = vec![Block::Paragraph(t(src))];
        assert_eq!(parse(&export(&doc)), doc, "{src}: {:?}", export(&doc));
    }
}

#[test]
fn a_code_block_holding_backticks_gets_a_longer_fence() {
    let doc = vec![Block::CodeBlock {
        lang: None,
        code: "```\ninner\n```\n".into(),
    }];
    assert_eq!(parse(&export(&doc)), doc);
}

#[test]
fn inline_code_with_backticks_round_trips() {
    let doc = vec![Block::Paragraph(vec![Inline::Code("a `b` c".into())])];
    assert_eq!(parse(&export(&doc)), doc);
}

#[test]
fn two_lists_in_a_row_stay_two_lists() {
    let item = |s: &str| Item {
        checked: None,
        blocks: vec![Block::Paragraph(t(s))],
    };
    let doc = vec![
        Block::List {
            start: None,
            items: vec![item("a")],
        },
        Block::List {
            start: None,
            items: vec![item("b")],
        },
    ];
    let back = parse(&export(&doc));
    assert_eq!(
        back.iter()
            .filter(|b| matches!(b, Block::List { .. }))
            .count(),
        2
    );
}

#[test]
fn empty_input_is_an_empty_document() {
    assert!(parse("").is_empty());
    assert_eq!(export(&[]), "");
}
