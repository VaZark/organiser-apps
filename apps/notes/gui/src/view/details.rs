//! The details of the selected note: title, text, colour, tags. Every edit is saved as it is made.

use iced::widget::{column, row};
use iced::{Alignment, Length};
use notes_schema::color::{Color, PRESETS};
use organiser_ui::Element;
use organiser_ui::components::action::action_button;
use organiser_ui::components::color_pick::color_swatch_with;
use organiser_ui::components::heart::heart;
use organiser_ui::components::toggle::IconToggle;
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
            // A note that is not saved yet has nothing to archive or to stick.
            if !draft {
                if self.sticky.usable() {
                    actions = actions.push(
                        IconToggle::new(Icon::Note, n.sticky)
                            .tooltip(if n.sticky {
                                "Remove the sticky"
                            } else {
                                "Show as a sticky"
                            })
                            .on_toggle(Message::DSticky),
                    );
                }
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
            row![
                entry("Title", d.title.as_str())
                    .on_input(Message::DTitle)
                    .width(Length::Fill),
                heart(n.favorite, Message::DFavorite(!n.favorite)),
                // The note's own colour, at the end of the title.
                self.colour_at_title_end(n.color, &d.color_text),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
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
            self.filing(n),
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

    /// Where the note is filed, and, in a folder that rates its notes, its stars.
    fn filing<'a>(&'a self, n: &'a notes_schema::note::Note) -> Element<'a, Message> {
        let mut col = column![].spacing(14).width(Length::Fill);
        let folder = self.folder_of(n);
        // The extras are the folder's choice: nothing to rate where ratings are off.
        if folder.is_some_and(|f| f.rated) {
            let mut stars = row![].spacing(2).align_y(Alignment::Center);
            for k in 1..=5u8 {
                let on = n.rating.is_some_and(|r| k <= r);
                // Pressing the star that is already the rating clears it.
                let next = if n.rating == Some(k) { None } else { Some(k) };
                stars = stars.push(
                    flat_button(text(if on { "★" } else { "☆" }).size(22))
                        .on_press(Message::DRating(next)),
                );
            }
            col = col.push(field("Rating", stars));
        }
        let here = self
            .folder_choices
            .iter()
            .find(|c| c.id == n.folder_id)
            .cloned();
        col = col.push(field(
            "Folder",
            dropdown(&self.folder_choices, here, Message::DFolder),
        ));
        // The folder's sections, when it has any.
        if !self.section_choices.is_empty() {
            let current = self
                .section_choices
                .iter()
                .find(|c| c.id == n.section)
                .cloned();
            col = col.push(field(
                "Section",
                dropdown(&self.section_choices, current, Message::DSection),
            ));
        }
        col.into()
    }

    /// The colour swatch that ends the title row: a picker with the presets, for any colour. A
    /// note without one has the default card, shown as the default swatch.
    fn colour_at_title_end<'a>(&'a self, current: Option<Color>, typed: &'a str) -> Element<'a, Message> {
        color_swatch_with(
            current.map_or(organiser_ui::components::color_pick::UNSET, paint),
            typed,
            22.0,
            PRESETS.iter().map(|(_, h)| *h),
            Message::DColorText,
            Message::DColorPick,
        )
    }
}
