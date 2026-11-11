//! A Markdown text field with a Write/Preview switch: the source in a text editor, or the note as
//! it reads. Ticking a task in the preview is reported, so the app can change the source.
//!
//! The document to draw is the app's to keep (parse once per change, not once per frame).

use iced::Length;
use iced::widget::{column, row, text_editor};

use crate::Element;
use crate::components::toggle::IconToggle;
use crate::iad::Theme;
use crate::iad::prelude::*;
use crate::iad::primitives::rich_text as rt;

pub struct MarkdownField<'a, M> {
    label: &'a str,
    content: &'a text_editor::Content,
    doc: &'a [rt::Block],
    theme: Theme,
    preview: bool,
    placeholder: &'a str,
    min_lines: usize,
    on_action: Box<dyn Fn(text_editor::Action) -> M + 'a>,
    on_preview: Box<dyn Fn(bool) -> M + 'a>,
    on_task: Box<dyn Fn(usize, bool) -> M + 'a>,
    on_link: Box<dyn Fn(String) -> M + 'a>,
}

impl<'a, M: Clone + 'a> MarkdownField<'a, M> {
    /// `doc` is `content` parsed and drawn-ready (see `components::rich::render`).
    pub fn new(
        label: &'a str,
        content: &'a text_editor::Content,
        doc: &'a [rt::Block],
        theme: Theme,
        on_action: impl Fn(text_editor::Action) -> M + 'a,
        on_preview: impl Fn(bool) -> M + 'a,
    ) -> Self {
        Self {
            label,
            content,
            doc,
            theme,
            preview: false,
            placeholder: "Write in Markdown…",
            min_lines: 8,
            on_action: Box::new(on_action),
            on_preview: Box::new(on_preview),
            on_task: Box::new(|_, _| unreachable!("on_task is set before the field is drawn")),
            on_link: Box::new(|_| unreachable!("on_link is set before the field is drawn")),
        }
    }

    pub fn preview(mut self, on: bool) -> Self {
        self.preview = on;
        self
    }

    pub fn placeholder(mut self, p: &'a str) -> Self {
        self.placeholder = p;
        self
    }

    pub fn min_lines(mut self, n: usize) -> Self {
        self.min_lines = n;
        self
    }

    /// A task checkbox in the preview was toggled: its number in reading order, and the new state.
    pub fn on_task(mut self, f: impl Fn(usize, bool) -> M + 'a) -> Self {
        self.on_task = Box::new(f);
        self
    }

    /// A link in the preview was pressed.
    pub fn on_link(mut self, f: impl Fn(String) -> M + 'a) -> Self {
        self.on_link = Box::new(f);
        self
    }
}

impl<'a, M: Clone + 'a> From<MarkdownField<'a, M>> for Element<'a, M> {
    fn from(f: MarkdownField<'a, M>) -> Self {
        let head = row![
            container(caption(f.label)).width(Length::Fill),
            IconToggle::new(Icon::Eye, f.preview)
                .tooltip(if f.preview {
                    "Back to writing"
                } else {
                    "Preview"
                })
                .on_toggle(f.on_preview),
        ]
        .align_y(iced::Alignment::Center);
        let body: Element<'a, M> = if f.preview {
            if f.doc.is_empty() {
                caption("Nothing to show yet.").into()
            } else {
                RichText::new(&f.theme, f.doc)
                    .on_link(f.on_link)
                    .on_task(f.on_task)
                    .into()
            }
        } else {
            text_view(f.content)
                .on_action(f.on_action)
                .placeholder(f.placeholder)
                .min_lines(f.min_lines)
                .max_lines(20)
                .into()
        };
        column![head, body].spacing(4).width(Length::Fill).into()
    }
}
