//! The task list: the nested tasks.

use iced::Length;
use iced::widget::{column, row};
use organiser_engine::Row;
use organiser_ui::Element;
use organiser_ui::components::chip::{chip, more, pill};
use organiser_ui::components::dot::color_from_hex;
use organiser_ui::components::empty::empty;
use organiser_ui::components::group_header::{UNASSIGNED, group_header};
use organiser_ui::components::icons;
use organiser_ui::components::row::selectable;
use organiser_ui::components::tag_groups::tag_groups;
use organiser_ui::components::tag_query::count as tag_count;
use organiser_ui::iad::prelude::*;
use organiser_ui::iad::primitives::drag::DropHint;
use organiser_ui::iad::primitives::tag_groups::GroupEvent;
use organiser_ui::primitives::expand::plain_rows;
use organiser_ui::primitives::groups::group_by;
use organiser_ui::primitives::tags::collapse;
use organiser_ui::primitives::text::ellipsize;
use organiser_ui::widgets::schema_filters::{Choice, active_filters};
use todo_schema::due::{Bucket, bucket, label};
use todo_schema::priority::Priority;
use todo_schema::task::Task;
use todo_schema::views::Scope;

use crate::app::{App, Message};

const INDENT: f32 = 24.0;
/// The most width the epic, date and tags take before they wrap.
const SIDE_WIDTH: f32 = 270.0;

impl App {
    /// `stacked`: the list is too narrow for chips beside the titles.
    pub(super) fn list_page(&self, stacked: bool) -> Element<'_, Message> {
        let rows = if self.is_flat() {
            plain_rows(&self.flat_tasks)
        } else {
            self.folds.rows(&self.forest, |t| t.id)
        };
        let body: Element<'_, Message> = if rows.is_empty() {
            let searching = self.search.query().is_some();
            let (title, text) = if searching || self.filter_count() > 0 {
                ("No matching tasks", "Try a different search or filter.")
            } else {
                ("Nothing to do", "Press + to add a task.")
            };
            empty(Icon::Check, title, text)
        } else {
            match self.epic_filter.filter(|_| self.scope == Scope::All) {
                Some(epic) if self.group_by_section => self.epic_sections_list(epic, &rows, stacked),
                // Not in sections: the tasks, pinned first, as anywhere else.
                Some(_) => self.pinned_and_rest(&rows, stacked),
                None if matches!(self.scope, Scope::All | Scope::Archived) => {
                    self.pinned_and_rest(&rows, stacked)
                }
                None => self.open_ground(rows.iter().map(|r| self.dnd_row(r, stacked)).collect()),
            }
        };
        let mut col = column![].spacing(12).width(Length::Fill);
        if let Some(epic) = self.epic_filter.filter(|_| self.scope == Scope::All) {
            col = col.push(self.epic_header(epic));
        }
        if let Some(menu) = self.all_page_menu() {
            col = col.push(menu);
        }
        if let Some(strip) = self.active_filters() {
            col = col.push(strip);
        }
        col.push(body).into()
    }

    /// The top of an epic's page: its colour, name and how far along it is, with a button to edit
    /// it and a menu to add a section.
    pub(super) fn epic_header(&self, id: i64) -> Element<'_, Message> {
        let Some(s) = self.epics.iter().find(|s| s.epic.id == id) else {
            return space().into();
        };
        let mut title = row![icons::tinted(
            icons::named(s.epic.icon.as_deref()),
            22.0,
            s.epic.color.as_deref().and_then(color_from_hex),
        )]
        .spacing(8)
        .align_y(iced::Alignment::Center);
        title = title.push(text(s.epic.title.clone()).size(20).width(Length::Fill));
        // The deadline, if it has one, beside the edit button.
        if let Some(d) = s.epic.due {
            let role = match bucket(d, self.today) {
                Bucket::Overdue => Role::Error,
                Bucket::Today => Role::Warning,
                _ => Role::Accent,
            };
            title = title.push(pill(&label(d, self.today), role));
        }
        title = title.push(tooltip(
            icon_button(Icon::Edit).on_press(Message::EditEpic(id)),
            "Edit epic",
            TooltipPosition::Bottom,
        ));
        title = title.push(
            menu_button(
                Icon::ViewMore,
                Menu::new("Epic")
                    .section([MenuItem::switch(
                        "Group by section",
                        self.group_by_section,
                        Message::GroupBySection(!self.group_by_section),
                    )])
                    .section([MenuItem::action("Add section", Message::NewSection(id))]),
            )
            .tooltip("More"),
        );
        column![
            title,
            progress_bar(0.0..=1.0, s.progress()),
            caption(format!("{} of {} tasks done", s.total - s.open, s.total)),
        ]
        .spacing(6)
        .padding([4, 4])
        .into()
    }

    /// An epic's rows under a collapsible header per section: the tasks in no section first, then
    /// each section in order, empty ones too. A subtask goes with its parent.
    fn epic_sections_list<'a>(
        &'a self,
        epic: i64,
        rows: &[Row<'a, Task>],
        stacked: bool,
    ) -> Element<'a, Message> {
        let sections = self
            .epic_sections
            .get(&epic)
            .map(Vec::as_slice)
            .unwrap_or_default();
        if sections.is_empty() {
            return self.open_ground(rows.iter().map(|r| self.dnd_row(r, stacked)).collect());
        }
        // Which group each top-level row opens: `None` is the tasks in no section.
        let mut groups: Vec<(Option<i64>, Vec<Element<'a, Message>>)> = std::iter::once(None)
            .chain(sections.iter().map(|s| Some(s.section.id)))
            .map(|id| (id, Vec::new()))
            .collect();
        let mut at = 0;
        for r in rows {
            if r.depth == 0 {
                let want = r
                    .value
                    .section
                    .filter(|s| sections.iter().any(|x| x.section.id == *s));
                at = groups.iter().position(|(id, _)| *id == want).unwrap_or(0);
            }
            // Hold a task and move to carry it to another section; its subtasks go with it.
            groups[at].1.push(self.dnd_row(r, stacked));
        }
        let dragging = self.dragging.is_some();
        let searching = !self.search.text().trim().is_empty() || self.filter_count() > 0;
        let mut col = column![].spacing(16).width(Length::Fill);
        for (id, items) in groups {
            let Some(sec) = id.and_then(|id| sections.iter().find(|s| s.section.id == id)) else {
                // The tasks in no section need no header. While dragging there is always a
                // place to drop one, to take it out of its section.
                if !items.is_empty() {
                    col = col.push(
                        drop_zone(boxed_list(items), dragging, Message::DropInSection(None))
                            .hint(DropHint::new("Make a separate task, in no section")),
                    );
                } else if dragging {
                    col = col.push(drop_zone(
                        container(caption("Drop here for no section"))
                            .padding(12)
                            .width(Length::Fill),
                        true,
                        Message::DropInSection(None),
                    )
                    .hint(DropHint::new("Make a separate task, in no section")));
                }
                continue;
            };
            // Searching or filtering shows only the sections that have a match.
            if items.is_empty() && searching {
                continue;
            }
            let sid = sec.section.id;
            let folded = self.folded_sections.contains(&sid);
            let header = row![
                icon_button(if folded { Icon::GoNext } else { Icon::GoDown })
                    .on_press(Message::SectionFold(sid)),
                // Read only: the name, and under it what the section is for, small. Press and
                // hold it to carry the section to another place.
                {
                    let mut name = column![text(sec.section.title.clone()).size(15)].spacing(1);
                    if let Some(line) = sec.section.notes.lines().map(str::trim).find(|l| !l.is_empty()) {
                        name = name.push(text(ellipsize(line, 110))
                                .size(11)
                                .style(organiser_ui::iad::styles::text::dim));
                    }
                    container(drag_source(name, Message::SectionDragStart(sid)).ghost_padding([4, 10]))
                        .width(Length::Fill)
                },
                caption(format!("{}/{}", sec.total - sec.open, sec.total)),
                tooltip(
                    icon_button(Icon::Edit).on_press(Message::Select(sid)),
                    "Edit section",
                    TooltipPosition::Bottom,
                ),
                tooltip(
                    icon_button(Icon::ListAdd).on_press(Message::NewInSection(sid)),
                    "Add a task to this section",
                    TooltipPosition::Bottom,
                ),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center);
            // The header and its tasks are one place to drop, folded or not.
            let mut group = column![header].spacing(8);
            if !folded {
                group = group.push(if items.is_empty() {
                    caption("No tasks in this section").into()
                } else {
                    boxed_list(items)
                });
            }
            // A task or a section can be dropped here.
            let hint = if self.dragging_section.is_some() {
                format!("Place it at {}", sec.section.title)
            } else {
                format!("Move to {}", sec.section.title)
            };
            col = col.push(
                drop_zone(
                    group,
                    dragging || self.dragging_section.is_some_and(|m| m != sid),
                    Message::DropInSection(Some(sid)),
                )
                .hint(DropHint::new(hint)),
            );
        }
        col.into()
    }

    /// The filters that are on, each a chip that removes itself when pressed.
    pub(super) fn active_filters(&self) -> Option<Element<'_, Message>> {
        if self.filter_count() == 0 {
            return None;
        }
        let mut tag_line: Option<Element<'_, Message>> = None;
        let mut chips = row![icon_button(Icon::Filter).on_press(Message::OpenFilter)]
            .spacing(4)
            .align_y(iced::Alignment::Center);
        // The groups: alternatives, with a comma between them.
        if tag_count(&self.tag_groups) > 0 {
            let groups: Vec<Vec<String>> = self.tag_groups.groups().to_vec();
            let named = groups.clone();
            tag_line = Some(tag_groups(&groups, move |g, t| {
                Message::TagGroups(GroupEvent::Toggle(g, named[g][t].clone()))
            }));
        }
        for c in active_filters(
            super::filter::kind(),
            &self.filters,
            &|f| match f.name {
                "status" => self
                    .statuses
                    .all()
                    .iter()
                    .map(|s| Choice {
                        value: serde_json::Value::from(s.id),
                        label: s.name.clone(),
                        color: None,
                    })
                    .collect(),
                _ => Vec::new(),
            },
            Message::Fields,
        ) {
            chips = chips.push(c);
        }
        // The tag groups read like the filter's input: on a line of their own, under the chips.
        Some(match tag_line {
            Some(t) => column![chips.wrap(), t].spacing(6).into(),
            None => chips.wrap().into(),
        })
    }

    /// A task row that can be picked up and dropped on: onto another task it becomes that task's
    /// subtask. Always wrapped the same way, so a drag in progress keeps its place in the tree.
    fn dnd_row<'a>(&'a self, r: &Row<'a, Task>, stacked: bool) -> Element<'a, Message> {
        let id = r.value.id;
        let source = drag_source(self.task_row(r, stacked), Message::DragStart(id));
        let active = self.dragging.is_some();
        if self.drag_blocked.contains(&id) {
            // Over itself or its own subtasks nothing happens, and nothing behind it does either.
            return drop_zone(source, active, Message::DragEnd).into();
        }
        drop_zone(source, active, Message::DropOnTask(id))
            .hint(
                DropHint::new(format!("Make a subtask of {}", ellipsize(&r.value.title, 28)))
                    .shrink(),
            )
            .into()
    }

    /// The `⋯` at the top right of the pages of every task: how they are grouped. `None` on an
    /// epic's page (which has its own) and on Current tasks.
    pub(super) fn all_page_menu(&self) -> Option<Element<'_, Message>> {
        if !matches!(self.scope, Scope::All | Scope::Archived)
            || self.epic_filter.is_some()
            || self.epics.is_empty()
        {
            return None;
        }
        Some(
            row![
                space().width(Length::Fill),
                menu_button(
                    Icon::ViewMore,
                    Menu::new("Page").section([MenuItem::switch(
                        "Group by epic",
                        self.group_by_epic,
                        Message::GroupByEpic(!self.group_by_epic),
                    )]),
                )
                .tooltip("More"),
            ]
            .align_y(iced::Alignment::Center)
            .into(),
        )
    }

    /// The heading of a group of tasks: the epic's icon in its colour and its name, with how many
    /// tasks (and a button to open the epic) when `count` is given; or "Unassigned" for the tasks
    /// in no epic.
    pub(super) fn epic_group_header(&self, epic: Option<i64>, count: Option<usize>) -> Element<'_, Message> {
        match epic.and_then(|id| self.epics.iter().find(|s| s.epic.id == id)) {
            Some(s) => group_header(
                Some((
                    icons::named(s.epic.icon.as_deref()),
                    s.epic.color.as_deref().and_then(color_from_hex),
                )),
                &s.epic.title,
                s.epic.notes.lines().map(str::trim).find(|l| !l.is_empty()),
                count,
                Some((Message::EditEpic(s.epic.id), "Edit epic")),
            ),
            None => group_header(None, UNASSIGNED, None, count, None),
        }
    }

    /// `rows` with the pinned tasks in a "Pinned" section on top, under their own heading. Then,
    /// when the page is grouped by epic, a group for each epic and the tasks in none last under
    /// "Unassigned"; otherwise the rest under "Others". A task's subtasks stay with it. Without
    /// pinned tasks or groups, just the list.
    fn pinned_and_rest<'a>(&'a self, rows: &[Row<'a, Task>], stacked: bool) -> Element<'a, Message> {
        // The rest, each row with the epic of the top-level task it belongs to, and whether it is
        // that top-level task.
        let (mut pinned, mut rest): (Vec<Element<'a, Message>>, Vec<(Option<i64>, bool, Element<'a, Message>)>) =
            (Vec::new(), Vec::new());
        let (mut in_pinned, mut epic) = (false, None);
        for r in rows {
            if r.depth == 0 {
                in_pinned = r.value.pinned;
                epic = r.value.epic;
            }
            let row = self.dnd_row(r, stacked);
            if in_pinned {
                pinned.push(row);
            } else {
                rest.push((epic, r.depth == 0, row));
            }
        }
        let grouped = self.grouped_by_epic();
        if pinned.is_empty() && !grouped {
            return self.open_ground(rest.into_iter().map(|r| r.2).collect());
        }
        let mut col = column![].spacing(8).width(Length::Fill);
        if !pinned.is_empty() {
            col = col.push(caption("Pinned")).push(self.open_ground(pinned));
        }
        if grouped {
            let order: Vec<i64> = self.epics.iter().map(|s| s.epic.id).collect();
            for (epic, items) in group_by(rest, |r| r.0, &order) {
                let tasks = items.iter().filter(|r| r.1).count();
                // The heading and its tasks are one place to drop: a task dropped there joins
                // the epic, on its own.
                let label = match epic.and_then(|id| self.epic_titles.get(&id)) {
                    Some(title) => format!("Move to {}", ellipsize(title, 28)),
                    None => "Take out of its epic".to_string(),
                };
                let group = column![
                    self.epic_group_header(epic, Some(tasks)),
                    boxed_list(items.into_iter().map(|r| r.2).collect::<Vec<_>>()),
                ]
                .spacing(8);
                col = col.push(
                    drop_zone(group, self.dragging.is_some(), Message::DropInEpic(epic))
                        .hint(DropHint::new(label)),
                );
            }
        } else if !rest.is_empty() {
            col = col.push(caption("Others")).push(self.open_ground(rest.into_iter().map(|r| r.2).collect()));
        }
        col.into()
    }

    /// `rows` as one list that is also open ground: dropped on it, a task leaves its parent.
    fn open_ground<'a>(&'a self, items: Vec<Element<'a, Message>>) -> Element<'a, Message> {
        drop_zone(
            boxed_list(items),
            self.dragging.is_some(),
            Message::DropOut,
        )
        .hint(DropHint::new("Make a separate task"))
        .into()
    }

    fn task_row<'a>(&'a self, r: &Row<'a, Task>, stacked: bool) -> Element<'a, Message> {
        let t = r.value;
        let status = self.statuses.resolve(t.status);
        let closed = status.is_some_and(|s| s.closed);
        let selected = self.details.as_ref().is_some_and(|d| d.id() == t.id);
        let fold: Element<'_, Message> = if r.has_children {
            let icon = if r.expanded {
                Icon::GoDown
            } else {
                Icon::GoNext
            };
            icon_button(icon).on_press(Message::Fold(t.id)).into()
        } else if self.is_flat() {
            // Nothing is nested, so there is nothing to line up with.
            space().width(Length::Fixed(0.0)).into()
        } else {
            space().width(Length::Fixed(32.0)).into()
        };
        let check = check_button("", closed).on_toggle(move |_| Message::Toggle(t.id));

        // The title, and under it what is not obvious from the checkbox: in progress, won't do,
        // a high priority.
        let mut note = row![].spacing(4);
        let mut has_note = false;
        let mut push = |e: Element<'a, Message>| {
            note = std::mem::replace(&mut note, row![]).push(e);
            has_note = true;
        };
        // Any status but the one a task starts in is named, in its own colour.
        if let Some(s) = status
            && self.statuses.first_open().map(|f| f.id) != Some(s.id)
        {
            // The colour is the dot before the chips; here just the name.
            push(text(s.name.clone()).size(13).style(organiser_ui::iad::styles::text::dim).into());
        }
        if t.priority.is_urgent() {
            let role = if t.priority == Priority::Highest {
                Role::Error
            } else {
                Role::Warning
            };
            push(pill(t.priority.label(), role));
        }
        let mut title = column![text(t.title.clone())].spacing(2);
        // Without nesting a subtask would lose its context, so say whose it is.
        if self.is_flat()
            && let Some(parent) = t.parent_id.and_then(|id| self.task_titles.get(&id))
        {
            title = title.push(caption(format!("in {}", ellipsize(parent, 40))));
        }
        if has_note {
            title = title.push(note);
        }
        // A row is one button (see `selectable`), so the title is just text that fills the line.
        let title = title.width(Length::Fill).padding([6, 8]);

        // On the side, always when set: the epic, the person's own tags, then the due date, last.
        let mut side = row![].spacing(4).align_y(iced::Alignment::Center);
        if let Some(flags) = self.flags_row(t) {
            side = side.push(flags);
        }
        // On an epic's own page, or in an epic's group, every task is in that epic: no chip to say
        // so.
        if self.epic_filter.is_none()
            && !self.grouped_by_epic()
            && let Some(e) = t.epic.and_then(|id| self.epic_titles.get(&id))
        {
            side = side.push(icons::folder_chip(e, t.epic.and_then(|id| self.epic_color(id)), t.epic.and_then(|id| self.epic_icon(id))));
        }
        // The section as a chip, unless the epic's page shows it as a heading.
        if !(self.epic_filter.is_some() && self.group_by_section)
            && let Some(sec) = self.section_chip(t)
        {
            side = side.push(sec);
        }
        let tags = collapse(&t.tags, 2);
        for g in &tags.shown {
            side = side.push(chip(g));
        }
        if tags.more > 0 {
            side = side.push(more(tags.more));
        }
        // The date is always last.
        if let Some(d) = t.due {
            let role = match (closed, bucket(d, self.today)) {
                (true, _) => Role::Success,
                (_, Bucket::Overdue) => Role::Error,
                (_, Bucket::Today) => Role::Warning,
                _ => Role::Accent,
            };
            side = side.push(pill(&label(d, self.today), role));
        }
        // The status as a dot, at the very end, after the date.
        if let Some(d) = self.status_dot(t) {
            side = side.push(d);
        }

        // Beside the title when there is room for both. Otherwise the chips go under it and wrap
        // there, so the title keeps the full width instead of breaking one word per line.
        let main: Element<'a, Message> = if stacked {
            column![title, side.wrap()]
                .spacing(4)
                .width(Length::Fill)
                .into()
        } else {
            row![title, container(side.wrap()).max_width(SIDE_WIDTH)]
                .spacing(6)
                .align_y(iced::Alignment::Center)
                .width(Length::Fill)
                .into()
        };
        let line = row![
            space().width(Length::Fixed(INDENT * r.depth as f32)),
            fold,
            check,
            main,
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center)
        // Room at both edges in every mode: in a flat list there is no fold arrow to space the
        // checkbox from the edge, and the chips must not touch the right one.
        .padding([4, 12]);
        selectable(line, selected, Message::Select(t.id))
    }
}
