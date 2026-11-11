//! The details of the selected note: title, text, colour, tags. Every edit is saved as it is made.

use iced::widget::{button, column, row};
use iced::{Alignment, Background, Border, Length};
use notes_schema::color::{Color, PRESETS};
use organiser_ui::Element;
use organiser_ui::components::action::action_button;
use organiser_ui::components::toggle::IconToggle;
use organiser_ui::iad::Theme;
use organiser_ui::iad::prelude::*;
use organiser_ui::widgets::markdown_field::MarkdownField;
use organiser_ui::widgets::tag_editor::tag_editor;

use crate::app::{App, Message, paint};

fn field<'a>(label: &'a str, control: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    column![caption(label), control.into()]
        .spacing(4)
        .width(Length::Fill)
        .into()
}

/// A round colour button; `None` is the default card, drawn as an outline.
fn swatch<'a>(color: Option<Color>, active: bool) -> Element<'a, Message> {
    let size = 28.0;
    button(space().width(size).height(size))
        .padding(0)
        .on_press(Message::DColor(color))
        .style(move |theme: &Theme, _| button::Style {
            background: color.map(|c| Background::Color(paint(c))),
            border: Border {
                color: if active {
                    theme.semantic.swatch(Role::Accent).color
                } else {
                    theme.semantic.swatch(Role::Neutral).color
                },
                width: if active { 3.0 } else { 1.0 },
                radius: (size / 2.0).into(),
            },
            ..button::Style::default()
        })
        .into()
}

impl App {
    pub(super) fn details_pane(&self) -> Element<'_, Message> {
        let Some(d) = &self.details else {
            return space().into();
        };
        let n = &d.note;
        let trashed = n.is_trashed();
        let draft = d.is_new();

        let mut actions = row![].spacing(4).align_y(Alignment::Center);
        if !trashed {
            actions = actions.push(
                IconToggle::new(Icon::Pin, n.pinned)
                    .tooltip(if n.pinned { "Unpin" } else { "Pin to the top" })
                    .on_toggle(Message::DPin),
            );
            // A note that is not saved yet has nothing to archive.
            if !draft {
                actions = actions.push(
                    IconToggle::new(Icon::Archive, n.archived)
                        .tooltip(if n.archived { "Unarchive" } else { "Archive" })
                        .on_toggle(Message::DArchive),
                );
            }
        }

        let mut col = column![
            row![
                container(caption(if draft {
                    "New note".to_string()
                } else {
                    format!("Note {}", n.id)
                }))
                .width(Length::Fill),
                actions,
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            entry("Title", d.title.as_str()).on_input(Message::DTitle),
            tag_editor(
                &n.tags,
                d.adding_tag,
                &d.tag_text,
                &self.tag_names,
                Message::DTagStart,
                Message::DTagText,
                &|t| Message::DTagAdd(t),
                &|t| Message::DTagRemove(t),
            ),
            MarkdownField::new(
                "Note",
                &d.body,
                &d.doc,
                self.theme(),
                Message::DBody,
                Message::DPreview,
            )
            .preview(d.preview)
            .placeholder("Write in Markdown…  # heading, **bold**, - [ ] task")
            .on_task(Message::DTask)
            .on_link(Message::OpenLink),
            field("Colour", self.colors(n.color, &d.color_text)),
        ]
        .spacing(14)
        .width(Length::Fill);

        if let Some(e) = &d.error {
            col = col.push(tag(e.clone(), Role::Error));
        }
        col = if draft {
            // The same page makes a note: nothing is saved until Create.
            let ready = !d.title.trim().is_empty();
            col.push(
                row![
                    flat_button(text("Cancel")).on_press(Message::CloseDetails),
                    action_button("Create note").on_press_maybe(ready.then_some(Message::DCreate)),
                ]
                .spacing(8),
            )
        } else if trashed {
            col.push(
                row![
                    flat_button(text("Restore")).on_press(Message::DRestore),
                    destructive_button(text("Delete forever")).on_press(Message::DPurge),
                ]
                .spacing(8),
            )
        } else {
            col.push(destructive_button(text("Move to trash")).on_press(Message::DTrash))
        };

        scrolled_window(clamp(
            container(organiser_ui::components::tab_scope::tab_scope(col)).padding(16),
            organiser_ui::widgets::details_dock::READABLE_WIDTH,
        ))
        .height(Length::Fill)
        .into()
    }

    /// Preset swatches, "none", and a picker for any other colour.
    fn colors<'a>(&'a self, current: Option<Color>, text: &'a str) -> Element<'a, Message> {
        let mut swatches = row![swatch(None, current.is_none())].spacing(8);
        for (_, hex) in PRESETS {
            let c = Color::parse(hex);
            swatches = swatches.push(swatch(c, c.is_some() && c == current));
        }
        let custom = ColorInput::new(
            self.color_value(),
            text,
            Message::DColorText,
            Message::DColorPick,
        )
        .swatches(PRESETS.iter().map(|(_, h)| *h));
        column![
            swatches.wrap(),
            row![custom, caption("Any colour")]
                .spacing(8)
                .align_y(Alignment::Center)
        ]
        .spacing(8)
        .into()
    }
}
