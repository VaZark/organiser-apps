//! The details of the selected task or epic. Every edit is saved as it is made.

use iced::Length;
use iced::widget::{column, row};
use organiser_ui::Element;
use organiser_ui::components::action::action_button;
use organiser_ui::components::dot::{color_from_hex, dot};
use organiser_ui::iad::prelude::*;
use organiser_ui::iad::primitives::date::Date;
use organiser_ui::widgets::markdown_field::MarkdownField;
use organiser_ui::widgets::tag_editor::tag_editor;
use todo_schema::priority::Priority;

use crate::app::{App, Details, EpicChoice, Message, Subject};

fn field<'a>(label: &'a str, control: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    column![caption(label), control.into()]
        .spacing(4)
        .width(Length::Fill)
        .into()
}

fn date_input<'a>(text: &'a str, on: fn(String) -> Message) -> Element<'a, Message> {
    let view = Date::parse(text).unwrap_or_else(Date::today);
    DateInput::new(text, view, on)
        .on_pick(move |d| on(d.format()))
        .into()
}

impl App {
    pub(super) fn details_pane(&self) -> Element<'_, Message> {
        let Some(d) = &self.details else {
            return space().into();
        };
        let (kind, status_id) = match &d.subject {
            Subject::Task(t) => ("Task", t.status),
            Subject::Epic(e) => ("Epic", e.status),
        };

        let mut col = column![
            // Closing the page is the header's back button.
            row![caption(if d.is_new() {
                "New task".to_string()
            } else {
                format!("{kind} {}", d.id())
            }),]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            entry("Title", d.title.as_str()).on_input(Message::DTitle),
            // Right under the name, with no heading: `tag ✕  tag ✕  +`.
            tag_editor(
                d.tags(),
                d.adding_tag,
                &d.tag_text,
                &self.tag_names,
                Message::DTagStart,
                Message::DTagText,
                &|t| Message::DTagAdd(t),
                &|t| Message::DTagRemove(t),
            ),
            row![
                field("Status", self.status_picker(status_id)),
                self.priority_or_progress(d),
            ]
            .spacing(12),
        ]
        .spacing(14)
        .width(Length::Fill);

        if let Subject::Task(t) = &d.subject {
            let current = self.epic_choices.iter().find(|c| c.id == t.epic).cloned();
            let current = current.or_else(|| self.epic_choices.first().cloned());
            col = col.push(field(
                "Epic",
                dropdown(&self.epic_choices, current, |c: EpicChoice| {
                    Message::DEpic(c)
                }),
            ));
        }

        col = col
            .push(
                row![
                    field("Start date", date_input(&d.start, Message::DStart)),
                    field("Due date", date_input(&d.due, Message::DDue)),
                ]
                .spacing(12),
            )
            .push(
                MarkdownField::new(
                    "Details",
                    &d.notes,
                    &d.doc,
                    self.theme(),
                    Message::DNotes,
                    Message::DPreview,
                )
                .preview(d.preview)
                .placeholder("Add details in Markdown…  - [ ] a task, **bold**, `code`")
                .min_lines(7)
                .on_task(Message::DTask)
                .on_link(Message::OpenLink),
            );

        // A task that is not saved yet has no subtasks to list or add.
        if matches!(&d.subject, Subject::Task(_)) && !d.is_new() {
            col = col.push(self.subtasks(d));
        }
        if let Some(e) = &d.error {
            col = col.push(tag(e.clone(), Role::Error));
        }
        col = if d.is_new() {
            // The same page makes a task: nothing is saved until Create.
            let ready = !d.title.trim().is_empty();
            col.push(
                row![
                    flat_button(text("Cancel")).on_press(Message::CloseDetails),
                    action_button("Create task").on_press_maybe(ready.then_some(Message::DCreate)),
                ]
                .spacing(8),
            )
        } else {
            col.push(
                destructive_button(text(if matches!(d.subject, Subject::Epic(_)) {
                    "Delete epic"
                } else {
                    "Delete task"
                }))
                .on_press(Message::DDelete),
            )
        };

        scrolled_window(clamp(
            container(organiser_ui::components::tab_scope::tab_scope(col)).padding(16),
            organiser_ui::widgets::details_dock::READABLE_WIDTH,
        ))
        .height(Length::Fill)
        .into()
    }

    /// The statuses to choose from, led by a dot in the current one's colour.
    fn status_picker(&self, current: Option<i64>) -> Element<'_, Message> {
        let now = self.statuses.resolve(current);
        let picker = dropdown(self.statuses.all(), now.cloned(), Message::DStatus);
        match now
            .and_then(|s| s.color.as_deref())
            .and_then(color_from_hex)
        {
            Some(c) => row![dot(c, 12.0), picker]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into(),
            None => picker.into(),
        }
    }

    /// A task's priority picker, or an epic's progress.
    fn priority_or_progress<'a>(&'a self, d: &'a Details) -> Element<'a, Message> {
        match &d.subject {
            Subject::Task(t) => field(
                "Priority",
                dropdown(&Priority::ALL, Some(t.priority), Message::DPriority),
            ),
            Subject::Epic(e) => {
                let summary = self.epics.iter().find(|s| s.epic.id == e.id);
                let (done, total, p) =
                    summary.map_or((0, 0, 0.0), |s| (s.total - s.open, s.total, s.progress()));
                field(
                    "Progress",
                    column![
                        progress_bar(0.0..=1.0, p),
                        caption(format!("{done} of {total} tasks done"))
                    ]
                    .spacing(4),
                )
            }
        }
    }

    fn subtasks<'a>(&'a self, d: &'a Details) -> Element<'a, Message> {
        let mut col = column![].spacing(6);
        for s in &d.subtasks {
            let id = s.id;
            col = col.push(
                row![
                    check_button("", self.statuses.is_closed(s.status))
                        .on_toggle(move |_| Message::Toggle(id)),
                    flat_button(text(s.title.clone())).on_press(Message::Select(id)),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            );
        }
        col = col.push(
            entry("Add a subtask", d.sub_text.as_str())
                .on_input(Message::DSubText)
                .on_submit(Message::DSubAdd)
                .left_section(Icon::ListAdd.view(16.0)),
        );
        field("Subtasks", col)
    }
}
