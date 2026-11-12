//! The cards, laid out like Google Keep: as many columns as fit, each card in the shortest one
//! so tall and short notes pack together, or a list like the Todo list. Pinned notes come first
//! under their own heading; in All and Favorites the rest can be in sections, a folder each, with
//! the notes in no folder last under "Unassigned". A card takes the note's own colour, else its
//! folder's.

use iced::widget::{column, row};
use iced::{Alignment, Length};
use notes_schema::note::Note;
use notes_schema::section::SectionSummary;
use notes_schema::views::Scope;
use organiser_ui::Element;
use organiser_ui::components::card::card;
use organiser_ui::components::chip::{chip, hashtag, more};
use organiser_ui::components::dot::dot;
use organiser_ui::components::group_header::{UNASSIGNED, group_header};
use organiser_ui::components::heart::heart;
use organiser_ui::components::icons;
use organiser_ui::components::list_item::{self, list_item};
use organiser_ui::components::empty::empty;
use organiser_ui::components::tag_groups::tag_groups;
use organiser_ui::components::tag_query::count as tag_count;
use organiser_ui::iad::Theme;
use organiser_ui::iad::prelude::*;
use organiser_ui::iad::primitives::drag::DropHint;
use organiser_ui::iad::primitives::tag_groups::GroupEvent;
use organiser_ui::primitives::groups::group_by;
use organiser_ui::primitives::tags::collapse;
use organiser_ui::primitives::text::plain_line;
use organiser_ui::widgets::masonry::{PAGE_PADDING, columns_for, masonry};
use organiser_ui::widgets::schema_filters::active_filters;

use crate::app::{App, Message, paint};

/// `★★★★☆`
pub fn stars(rating: u8) -> String {
    let r = rating.min(5) as usize;
    format!("{}{}", "★".repeat(r), "☆".repeat(5 - r))
}

/// The tallest the drawn body of a card gets.
const CARD_BODY: f32 = 170.0;

/// A rough height for packing: a title line plus the snippet's lines and a row for tags.
fn weight(n: &Note) -> usize {
    2 + n
        .body
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count()
        .min(6)
        + usize::from(!n.tags.is_empty())
}

impl App {
    pub(super) fn grid_page<'a>(&'a self, width: f32) -> Element<'a, Message> {
        let theme = self.theme();
        let mut page = column![].spacing(16).width(Length::Fill);
        // The pages of every note say how they are grouped at their top right.
        if matches!(
            self.scope,
            Scope::Notes | Scope::Favorites | Scope::Archive | Scope::Trash
        ) && !self.folders.is_empty()
        {
            page = page.push(self.page_menu());
        }
        if self.scope == Scope::Trash && !self.notes.is_empty() {
            page = page.push(
                row![
                    container(caption(format!(
                        "Notes in the trash are deleted after {} days.",
                        self.prefs.settings.notes.trash_days
                    )))
                    .width(Length::Fill),
                    destructive_button(text("Empty trash")).on_press(Message::EmptyTrash),
                ]
                .align_y(Alignment::Center),
            );
        }

        // A folder's page starts with the folder: its icon, name, a button for its settings and a
        // menu to add a section.
        if let Scope::Folder(id) = self.scope {
            page = page.push(self.folder_page_header(id));
        }
        if let Some(strip) = self.active_filters() {
            page = page.push(strip);
        }

        let sectioned = self.sections_here();
        if self.notes.is_empty() && sectioned.is_none() {
            let (icon, title, hint) = match (self.scope, self.search.query()) {
                (_, Some(_)) => (Icon::Search, "No notes found", "Try another word."),
                (Scope::Notes, _) => (Icon::Note, "No notes yet", "Notes you add appear here."),
                (Scope::Favorites, _) => (
                    Icon::Heart,
                    "No favourites yet",
                    "Tap the heart on anything you love.",
                ),
                (Scope::Folder(_), _) => (
                    Icon::Folder,
                    "Nothing in this folder",
                    "Add a note with the + above.",
                ),
                (Scope::Archive, _) => (
                    Icon::Archive,
                    "Nothing archived",
                    "Archived notes stay out of the way.",
                ),
                (Scope::Trash, _) => (
                    Icon::Delete,
                    "The trash is empty",
                    "Deleted notes wait here for a while.",
                ),
            };
            page = page.push(empty(icon, title, hint));
        } else {
            let (pinned, others) = self.pinned();
            // A block of notes: a list or a grid of cards.
            let cols = columns_for(width);
            let stacked = list_item::stacked(width);
            let block = |notes: &[&'a Note]| -> Element<'a, Message> {
                if self.list_view {
                    boxed_list(notes.iter().map(|n| self.note_row(n, stacked)))
                } else {
                    self.masonry(&theme, notes, cols)
                }
            };
            // A folder with sections: the notes in none first, then each section under its header,
            // each a place to drop a note dragged from another.
            if let Some(sections) = sectioned {
                let dragging = self.dragging.is_some();
                let ids: Vec<i64> = sections.iter().map(|s| s.section.id).collect();
                let loose: Vec<&'a Note> = self
                    .notes
                    .iter()
                    .filter(|n| n.section.is_none_or(|s| !ids.contains(&s)))
                    .collect();
                if !loose.is_empty() || dragging {
                    let body = if loose.is_empty() {
                        container(caption("Drop here to take it out of its section"))
                            .padding(12)
                            .width(Length::Fill)
                            .into()
                    } else {
                        block(&loose)
                    };
                    page = page.push(
                        drop_zone(body, dragging, Message::DropInSection(None))
                            .hint(DropHint::new("Take out of its section")),
                    );
                }
                for s in sections {
                    let id = s.section.id;
                    let mine: Vec<&'a Note> =
                        self.notes.iter().filter(|n| n.section == Some(id)).collect();
                    let mut group = column![self.section_header(s)].spacing(8);
                    if !self.folded_sections.contains(&id) {
                        group = group.push(if mine.is_empty() {
                            caption("No notes in this section").into()
                        } else {
                            block(&mine)
                        });
                    }
                    // A note or a section can be dropped here.
                    let hint = if self.dragging_section.is_some() {
                        format!("Place it at {}", s.section.title)
                    } else {
                        format!("Move to {}", s.section.title)
                    };
                    page = page.push(
                        drop_zone(
                            group,
                            dragging || self.dragging_section.is_some_and(|m| m != id),
                            Message::DropInSection(Some(id)),
                        )
                        .hint(DropHint::new(hint)),
                    );
                }
            } else {
            // In All and Favorites the notes can be in sections, one per folder.
            let grouped = self.grouped_by_folder();
            if !pinned.is_empty() {
                page = page.push(caption("Pinned")).push(block(&pinned));
            }
            if grouped {
                // Pinned first, then a section per folder, then those in none.
                let order: Vec<i64> = self.folders.iter().map(|s| s.folder.id).collect();
                let home = self.default_folder_id();
                for (folder, notes) in group_by(others, |n| n.folder_id.or(home), &order) {
                    page = page.push(self.folder_header(folder, notes.len()));
                    page = page.push(block(&notes));
                }
            } else if !others.is_empty() {
                if !pinned.is_empty() {
                    page = page.push(caption("Others"));
                }
                page = page.push(block(&others));
            }
            }
        }
        // A list is kept to a readable width, as in Todo; the cards use all of it.
        let page: Element<'_, Message> = if self.list_view {
            organiser_ui::iad::prelude::page(page).maximum_size(1000.0).into()
        } else {
            page.into()
        };
        scrolled_window(container(page).padding(PAGE_PADDING))
            .height(Length::Fill)
            .into()
    }

    /// The filters that are on, each a chip that removes itself when pressed.
    fn active_filters(&self) -> Option<Element<'_, Message>> {
        if self.filter_count() == 0 {
            return None;
        }
        let mut tag_line: Option<Element<'_, Message>> = None;
        let mut chips = row![icon_button(Icon::Filter).on_press(Message::OpenFilter)]
            .spacing(4)
            .align_y(Alignment::Center);
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
            &|_| Vec::new(),
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

    /// `notes` dealt into `cols` columns, each card to the lightest column so far.
    fn masonry<'a>(
        &'a self,
        theme: &Theme,
        notes: &[&'a Note],
        cols: usize,
    ) -> Element<'a, Message> {
        masonry(notes.iter().map(|n| (weight(n), self.card(theme, n))), cols)
    }

    /// A note as a row of the list, laid out like the Todo list: the title over a line of the note,
    /// and beside them (under them in a narrow list) the chips: its colour as a dot, a sticky, its
    /// tags.
    fn note_row<'a>(&'a self, n: &'a Note, stacked: bool) -> Element<'a, Message> {
        let selected = self.details.as_ref().is_some_and(|d| d.note.id == n.id);
        let mut title = column![text(n.title.clone())].spacing(2);
        let body = plain_line(&n.body, 120);
        if !body.is_empty() {
            title = title.push(text(body).size(13).style(organiser_ui::iad::styles::text::dim));
        }
        let mut chips = row![].spacing(4).align_y(Alignment::Center);
        let mut any = false;
        // The note's own colour, as a dot before the chips.
        if let Some(c) = n.color {
            chips = chips.push(dot(paint(c), 9.0));
            any = true;
        }
        if let Some(r) = n.rating {
            chips = chips.push(text(stars(r)).size(13));
            any = true;
        }
        if let Some(f) = self.folder_chip_of(n) {
            chips = chips.push(f);
            any = true;
        }
        if let Some(sec) = self.section_chip_of(n) {
            chips = chips.push(sec);
            any = true;
        }
        if n.sticky {
            chips = chips.push(
                Chip::from_icon(Icon::Note)
                    .role(Role::Warning)
                    .size(ChipSize::Small),
            );
            any = true;
        }
        let tags = collapse(&n.tags, 2);
        for g in &tags.shown {
            chips = chips.push(chip(g));
            any = true;
        }
        if tags.more > 0 {
            chips = chips.push(more(tags.more));
            any = true;
        }
        self.draggable(
            n,
            list_item(
                title,
                any.then(|| chips.wrap().into()),
                Some(heart(n.favorite, Message::Fave(n.id))),
                stacked,
                selected,
                Message::Select(n.id),
            ),
        )
    }

    /// The sections of the folder whose page this is, if it has any: its notes are then shown in
    /// them.
    pub(super) fn sections_here(&self) -> Option<&Vec<SectionSummary>> {
        match self.scope {
            Scope::Folder(id) if self.group_by_section => {
                self.folder_sections.get(&id).filter(|v| !v.is_empty())
            }
            _ => None,
        }
    }

    /// Whether the views of every note (All, Favorites, Archive, Trash) are in groups, one per
    /// folder: the option is on and there are folders.
    pub(super) fn grouped_by_folder(&self) -> bool {
        self.group_by_folder
            && matches!(
                self.scope,
                Scope::Notes | Scope::Favorites | Scope::Archive | Scope::Trash
            )
            && !self.folders.is_empty()
    }

    /// The `⋯` at the top right of a page of every note: how it is grouped.
    fn page_menu(&self) -> Element<'_, Message> {
        container(
            row![
                space().width(Length::Fill),
                menu_button(
                    Icon::ViewMore,
                    Menu::new("Page").section([MenuItem::switch(
                        "Group by folder",
                        self.group_by_folder,
                        Message::GroupByFolder(!self.group_by_folder),
                    )]),
                )
                .tooltip("More"),
            ]
            .align_y(Alignment::Center),
        )
        .into()
    }

    /// A note that can be picked up, on a page of sections, to carry it to another.
    fn draggable<'a>(&self, note: &Note, e: Element<'a, Message>) -> Element<'a, Message> {
        if self.sections_here().is_some() {
            drag_source(e, Message::DragStart(note.id)).into()
        } else {
            e
        }
    }

    /// The top of a folder's page: its icon in its colour, its name over what it is for, how many
    /// notes, a button for its settings and, beside it, a menu to group by section and to add one.
    fn folder_page_header(&self, id: i64) -> Element<'_, Message> {
        let Some(s) = self.folders.iter().find(|s| s.folder.id == id) else {
            return space().into();
        };
        let mut name = column![text(s.folder.title.clone()).size(20)].spacing(2);
        if let Some(line) = s.folder.notes.lines().map(str::trim).find(|l| !l.is_empty()) {
            name = name.push(
                text(organiser_ui::primitives::text::ellipsize(line, 140))
                    .size(12)
                    .style(organiser_ui::iad::styles::text::dim),
            );
        }
        let head = row![
            icons::tinted(
                icons::named(s.folder.icon.as_deref()),
                22.0,
                Some(self.folder_color(&s.folder)),
            ),
            container(name).width(Length::Fill),
            caption(format!("{} notes", s.notes)),
            tooltip(
                icon_button(Icon::Edit).on_press(Message::OpenFolderSettings(id)),
                "Folder settings",
                TooltipPosition::Bottom,
            ),
            menu_button(
                Icon::ViewMore,
                Menu::new("Folder")
                    .section([MenuItem::switch(
                        "Group by section",
                        self.group_by_section,
                        Message::GroupBySection(!self.group_by_section),
                    )])
                    .section([MenuItem::action("Add section", Message::NewSection(id))]),
            )
            .tooltip("More"),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        container(head).padding([4, 4]).into()
    }

    /// The heading of a section: a fold arrow, its name over what it is for (small), how many
    /// notes, a button to edit it and one to add a note to it.
    fn section_header<'a>(&'a self, s: &'a SectionSummary) -> Element<'a, Message> {
        let id = s.section.id;
        let folded = self.folded_sections.contains(&id);
        let mut name = column![text(s.section.title.clone()).size(15)].spacing(1);
        if let Some(line) = s.section.notes.lines().map(str::trim).find(|l| !l.is_empty()) {
            name = name.push(
                text(organiser_ui::primitives::text::ellipsize(line, 110))
                    .size(11)
                    .style(organiser_ui::iad::styles::text::dim),
            );
        }
        let mut head = row![
            icon_button(if folded { Icon::GoNext } else { Icon::GoDown })
                .on_press(Message::SectionFold(id)),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        if let Some(c) = s.section.color {
            head = head.push(dot(paint(c), 10.0));
        }
        // Press and hold the name to carry the section to another place.
        head.push(
            container(drag_source(name, Message::SectionDragStart(id))).width(Length::Fill),
        )
            .push(caption(s.notes.to_string()))
            .push(tooltip(
                icon_button(Icon::Edit).on_press(Message::OpenSectionSettings(id)),
                "Edit section",
                TooltipPosition::Bottom,
            ))
            .push(tooltip(
                icon_button(Icon::ListAdd).on_press(Message::NewInSection(id)),
                "Add a note to this section",
                TooltipPosition::Bottom,
            ))
            .into()
    }

    /// The heading of a section: the folder's icon in its colour, its name, how many notes it
    /// shows and a button for its settings; or "Unassigned" for the notes in no folder.
    fn folder_header<'a>(&'a self, folder: Option<i64>, count: usize) -> Element<'a, Message> {
        match folder.and_then(|id| self.folders.iter().find(|s| s.folder.id == id)) {
            Some(s) => group_header(
                Some((icons::named(s.folder.icon.as_deref()), Some(self.folder_color(&s.folder)))),
                &s.folder.title,
                s.folder.notes.lines().map(str::trim).find(|l| !l.is_empty()),
                Some(count),
                Some((Message::OpenFolderSettings(s.folder.id), "Folder settings")),
            ),
            None => group_header(None, UNASSIGNED, None, Some(count), None),
        }
    }

    /// The section a note is in, as a chip, wherever the section's own heading is not shown: out of
    /// a folder's page, and on it when the notes are not grouped by section.
    fn section_chip_of<'a>(&'a self, n: &'a Note) -> Option<Element<'a, Message>> {
        if self.sections_here().is_some() {
            return None;
        }
        let sec = self.section_of(n)?;
        let chip = Chip::new(organiser_ui::primitives::text::ellipsize(&sec.title, 24))
            .size(ChipSize::Small);
        Some(match sec.color {
            Some(c) => chip.color(paint(c)).into(),
            None => chip.role(Role::Neutral).into(),
        })
    }

    /// Where a note is filed, as a chip, when the notes are not in sections that say so.
    fn folder_chip_of<'a>(&'a self, n: &'a Note) -> Option<Element<'a, Message>> {
        if self.grouped_by_folder() || matches!(self.scope, Scope::Folder(_)) {
            return None;
        }
        let f = self.folder_of(n).filter(|f| !f.is_default)?;
        Some(icons::folder_chip(&f.title, Some(self.folder_color(f)), f.icon.as_deref()))
    }

    fn card<'a>(&'a self, theme: &Theme, n: &'a Note) -> Element<'a, Message> {
        let selected = self.details.as_ref().is_some_and(|d| d.note.id == n.id);
        // The title, with the heart at the right edge: filled for a favourite, an outline if not.
        let mut body = column![
            row![
                text(n.title.clone()).size(15).width(Length::Fill),
                heart(n.favorite, Message::Fave(n.id))
            ]
            .align_y(Alignment::Start)
        ]
        .spacing(6);
        if let Some(r) = n.rating {
            body = body.push(text(stars(r)).size(14));
        }
        if let Some(doc) = self.docs.get(&n.id) {
            // The note as it reads, clipped: a card is a preview, not the whole note.
            body = body.push(
                container(RichText::new(theme, doc).compact(true).max_blocks(6))
                    .max_height(CARD_BODY)
                    .clip(true),
            );
        }
        let tags = collapse(&n.tags, 3);
        if !tags.shown.is_empty() {
            let mut chips = row(tags.shown.iter().map(|g| hashtag(g))).spacing(4);
            if tags.more > 0 {
                chips = chips.push(more(tags.more));
            }
            body = body.push(chips.wrap());
        }
        if let Some(f) = self.folder_chip_of(n) {
            body = body.push(f);
        }
        if let Some(sec) = self.section_chip_of(n) {
            body = body.push(sec);
        }
        self.draggable(n, card(body, self.note_tint(n), selected, Message::Select(n.id)))
    }
}
