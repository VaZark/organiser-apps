//! The details of the selected task or epic. Every edit is saved as it is made.

use iced::Length;
use iced::widget::{column, row};
use organiser_ui::Element;
use organiser_ui::components::action::action_button;
use organiser_ui::components::dot::{color_from_hex, dot};
use organiser_ui::components::icon_pick::icon_color_picker;
use organiser_ui::components::toggle::IconToggle;
use organiser_ui::iad::prelude::*;
use organiser_ui::iad::primitives::color_picker::Hsva;
use organiser_ui::iad::primitives::date::Date;
use organiser_ui::widgets::markdown_field::MarkdownField;
use organiser_ui::widgets::tag_editor::tag_editor;
use todo_schema::priority::Priority;

use super::statuses::SWATCHES;
use crate::app::{App, Details, Message, Subject};

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
            Subject::Epic(_) => ("Epic", None),
            Subject::Section(_) => ("Section", None),
        };
        let section = matches!(d.subject, Subject::Section(_));

        // The colour of an epic or section sits at the right end of the title row.
        let title: Element<'_, Message> = match self.color_of(d) {
            Some(color) => row![
                entry("Title", d.title.as_str())
                    .on_input(Message::DTitle)
                    .width(Length::Fill),
                // An epic's icon and colour in one picker, at the end of the title; a section has
                // a colour only.
                self.icon_color_input(d, color),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into(),
            None => entry("Title", d.title.as_str())
                .on_input(Message::DTitle)
                .into(),
        };

        let mut col = column![
            // Closing the page is the header's back button.
            row![
                container(caption(if d.is_new() {
                    "New task".to_string()
                } else {
                    format!("{kind} {}", d.id())
                }))
                .width(Length::Fill),
                self.more_menu(d),
                self.pin_toggle(d),
                self.sticky_toggle(d),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            title,
        ]
        .spacing(14)
        .width(Length::Fill);

        if !section {
            col = col.push(
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
            );
            // Only a task has a status and a priority: an epic has its progress.
            if let Subject::Task(t) = &d.subject {
                col = col.push(
                    row![
                        field("Status", self.status_picker(status_id)),
                        field(
                            "Priority",
                            dropdown(&Priority::ALL, Some(t.priority), Message::DPriority),
                        ),
                    ]
                    .spacing(12),
                );
            }
        }
        // How far along, across the whole page.
        if let Some(p) = self.progress_of(d) {
            col = col.push(p);
        }
        if !section {
            col = col.push(
                row![
                    field("Start date", date_input(&d.start, Message::DStart)),
                    field("Due date", date_input(&d.due, Message::DDue)),
                ]
                .spacing(12),
            );
        }
        col = col.push(
            MarkdownField::new(
                if section { "Description" } else { "Details" },
                &d.notes,
                &d.doc,
                self.theme(),
                Message::DNotes,
                Message::DPreview,
            )
            .preview(d.preview)
            .placeholder("Add details in Markdown…  - [ ] a task, **bold**, `code`")
            .min_lines(if section { 4 } else { 7 })
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
                destructive_button(text(match d.subject {
                    Subject::Epic(_) => "Delete epic",
                    Subject::Section(_) => "Delete section",
                    Subject::Task(_) => "Delete task",
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

    /// The statuses to choose from, each led by a dot in its own colour, in the list and on the
    /// button.
    fn status_picker(&self, current: Option<i64>) -> Element<'_, Message> {
        let now = self.statuses.resolve(current);
        dropdown_lead(
            self.statuses.all(),
            now.cloned(),
            Message::DStatus,
            // The dot is 12 wide.
            12.0,
            |s: &todo_schema::status::Status| {
                s.color
                    .as_deref()
                    .and_then(color_from_hex)
                    .map(|c| dot(c, 12.0))
            },
        )
        .into()
    }

    /// The colour of an epic or section as `#rrggbb`, `Some` (even when unset) for the subjects
    /// that have one.
    fn color_of<'a>(&self, d: &'a Details) -> Option<&'a str> {
        let hex = match &d.subject {
            Subject::Epic(e) => e.color.as_deref(),
            Subject::Section(s) => s.color.as_deref(),
            Subject::Task(_) => return None,
        };
        Some(hex.unwrap_or("#9aa0a6"))
    }

    /// An epic's or section's progress, across the page.
    fn progress_of(&self, d: &Details) -> Option<Element<'_, Message>> {
        let (done, total, p) = match &d.subject {
            Subject::Epic(e) => {
                let s = self.epics.iter().find(|s| s.epic.id == e.id)?;
                (s.total - s.open, s.total, s.progress())
            }
            Subject::Section(sec) => {
                let s = self
                    .epic_sections
                    .get(&sec.epic)?
                    .iter()
                    .find(|s| s.section.id == sec.id)?;
                (s.total - s.open, s.total, s.progress())
            }
            Subject::Task(_) => return None,
        };
        Some(
            field(
                "Progress",
                column![
                    progress_bar(0.0..=1.0, p),
                    caption(format!("{done} of {total} tasks done"))
                ]
                .spacing(4),
            ),
        )
    }

    fn subtasks<'a>(&'a self, d: &'a Details) -> Element<'a, Message> {
        let mut col = column![].spacing(6);
        for s in &d.subtasks {
            let id = s.id;
            col = col.push(
                row![
                    check_button("", self.statuses.is_closed(s.status))
                        .on_toggle(move |_| Message::Toggle(id)),
                    flat_button(text(s.title.clone()))
                        .on_press(Message::Select(id))
                        .width(Length::Fill),
                    tooltip(
                        icon_button(Icon::GoUp).on_press(Message::DConvert(id)),
                        "Convert to task",
                        TooltipPosition::Bottom,
                    ),
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

impl App {
    /// The `:` menu beside the sticky toggle: where a task lives (its epic and section, or
    /// converting a subtask to a task), and what an epic can add (a section). Out of the form,
    /// which is left for what the thing is.
    fn more_menu(&self, d: &Details) -> Element<'_, Message> {
        match &d.subject {
            Subject::Task(t) => self.task_menu(t, d.is_new()),
            Subject::Epic(e) => menu_button(
                Icon::ViewMore,
                Menu::new("Epic").section([MenuItem::action(
                    "Add section",
                    Message::NewSection(e.id),
                )]),
            )
            .tooltip("More")
            .into(),
            Subject::Section(_) => space().into(),
        }
    }

    /// What a task can do about where it lives, and only what applies: a subtask lives where its
    /// parent does, so it can only be converted to a task; a task can move to an epic, and to a
    /// section once its epic has some. No menu when there is nothing to offer.
    fn task_menu(&self, t: &todo_schema::task::Task, draft: bool) -> Element<'_, Message> {
        let mut menu = Menu::new("Task");
        if t.parent_id.is_some() {
            if draft {
                return space().into();
            }
            menu = menu.section([MenuItem::action("Convert to task", Message::DConvert(t.id))
                .icon(Icon::GoUp)]);
        } else {
            let mut placement = vec![MenuItem::submenu(
                "Move to epic",
                Menu::new("Move to epic")
                    .section(self.epic_choices.iter().map(|c| {
                        MenuItem::select(c.title.clone(), c.id == t.epic, Message::DEpic(c.clone()))
                    })),
            )];
            // Sections belong to an epic.
            if let Some(sections) = t
                .epic
                .and_then(|e| self.epic_sections.get(&e))
                .filter(|v| !v.is_empty())
            {
                let none = MenuItem::select(
                    "No section",
                    t.section.is_none(),
                    Message::DSection(None),
                );
                placement.push(MenuItem::submenu(
                    "Move to section",
                    Menu::new("Move to section")
                        .section(std::iter::once(none).chain(sections.iter().map(|s| {
                            MenuItem::select(
                                s.section.title.clone(),
                                t.section == Some(s.section.id),
                                Message::DSection(Some(s.section.id)),
                            )
                        }))),
                ));
            }
            menu = menu.section(placement);
        }
        menu_button(Icon::ViewMore, menu).tooltip("More").into()
    }

    /// The picker at the end of the title: an epic's icon and colour together; a section's colour.
    fn icon_color_input<'a>(&'a self, d: &'a Details, hex: &'a str) -> Element<'a, Message> {
        let Subject::Epic(e) = &d.subject else {
            return self.color_input(d.id(), hex);
        };
        let rgb = color_from_hex(hex).unwrap_or(organiser_ui::components::color_pick::UNSET);
        icon_color_picker(
            e.icon.as_deref(),
            rgb,
            self.status_text.get(&d.id()).map_or(hex, String::as_str),
            22.0,
            Message::DIcon,
            Message::DColorText,
            Message::DColor,
        )
    }

    /// A colour swatch to pick or type a colour from, for the epic or section `id`.
    fn color_input<'a>(&'a self, id: i64, hex: &'a str) -> Element<'a, Message> {
        let rgb = color_from_hex(hex).unwrap_or(iced::Color::from_rgb8(0x9a, 0xa0, 0xa6));
        let current = Hsva::from_rgb8(
            (rgb.r * 255.0) as u8,
            (rgb.g * 255.0) as u8,
            (rgb.b * 255.0) as u8,
        );
        let typed = self.status_text.get(&id).map_or(hex, String::as_str);
        ColorInput::new(current, typed, Message::DColorText, Message::DColor)
            .swatches(SWATCHES)
            .trigger(dot(rgb, 22.0))
            .into()
    }

    /// The pin toggle: keeps a task ahead of the others in every view.
    fn pin_toggle(&self, d: &Details) -> Element<'_, Message> {
        let Subject::Task(t) = &d.subject else {
            return space().into();
        };
        IconToggle::new(Icon::Pin, t.pinned)
            .tooltip(if t.pinned { "Unpin" } else { "Pin to the top" })
            .on_toggle(Message::DPin)
            .into()
    }

    /// The "show as a sticky" toggle: for a saved task, once Sticky Notes is linked.
    fn sticky_toggle(&self, d: &Details) -> Element<'_, Message> {
        let Subject::Task(t) = &d.subject else {
            return space().into();
        };
        if !self.sticky.usable() || d.is_new() {
            return space().into();
        }
        IconToggle::new(Icon::Note, t.sticky)
            .tooltip(if t.sticky {
                "Remove the sticky"
            } else {
                "Show as a sticky"
            })
            .on_toggle(Message::DSticky)
            .into()
    }
}
