//! The cards, laid out like Google Keep: as many columns as fit, each card in the shortest one
//! so tall and short notes pack together, or a list like the Todo list. Pinned notes come first
//! under their own heading; in All and Favorites the rest can be in sections, a folder each, with
//! the notes in no folder last under "Unassigned". A card takes the note's own colour, else its
//! folder's.

use iced::widget::{column, row};
use iced::{Alignment, Length};
use notes_schema::note::Note;
use notes_schema::folder::FolderSummary;
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

/// `1 note`, `2 notes`.
fn notes_word(n: usize) -> String {
    format!("{n} note{}", if n == 1 { "" } else { "s" })
}

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
        // menu to add a folder in it.
        if let Scope::Folder(id) = self.scope {
            page = page.push(self.folder_page_header(id));
        }
        if let Some(strip) = self.active_filters() {
            page = page.push(strip);
        }

        let sectioned = self.sections_here();
        let searching = !self.search.text().trim().is_empty() || self.filter_count() > 0;
        // A section's sections as cards on top, when not every note below is listed and they are
        // not sections of the page.
        let top_cards = match self.scope {
            Scope::Folder(here) if !self.group_by_section && !self.show_all_notes && !searching => {
                self.subfolder_entries(here)
            }
            _ => None,
        };
        if self.notes.is_empty() && sectioned.is_none() && top_cards.is_none() {
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
                    "Nothing in this section",
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
            // A folder showing the folders in it as sections: its own notes first, then each
            // folder in it under its heading, each a place to drop a note dragged from another.
            if let Some(cards) = top_cards {
                page = page.push(cards);
            }
            if let (Scope::Folder(here), Some(subs)) = (self.scope, sectioned) {
                let dragging = self.dragging.is_some();
                let loose: Vec<&'a Note> = self.notes.iter().collect();
                if !loose.is_empty() {
                    page = page.push(
                        drop_zone(block(&loose), dragging, Message::DropInFolder(Some(here)))
                            .hint(DropHint::new("Move here")),
                    );
                }
                for s in subs {
                    let id = s.folder.id;
                    let mine: Vec<&'a Note> = self
                        .sub_notes
                        .get(&id)
                        .map(|v| v.iter().collect())
                        .unwrap_or_default();
                    // Searching or filtering shows only the folders that have a match.
                    if mine.is_empty() && searching {
                        continue;
                    }
                    let mut group = column![self.subfolder_header(s, mine.len())].spacing(8);
                    if !self.folded_sections.contains(&id) {
                        // Deeper than one level: the sections in it, as cards to open, before its
                        // notes.
                        // (Not when every note below is listed: they are in the list.)
                        let deeper = if searching || self.show_all_notes {
                            None
                        } else {
                            self.subfolder_entries(id)
                        };
                        let has_cards = deeper.is_some();
                        if let Some(deeper) = deeper {
                            group = group.push(deeper);
                        }
                        if !mine.is_empty() {
                            group = group.push(block(&mine));
                        } else if !has_cards {
                            group = group.push(caption("No notes in this section"));
                        }
                    }
                    // A note or a folder can be dropped here.
                    let hint = if self.dragging_folder.is_some() {
                        format!("Move into {}", s.folder.title)
                    } else {
                        format!("Move to {}", s.folder.title)
                    };
                    page = page.push(
                        drop_zone(
                            group,
                            dragging || self.dragging_folder.is_some_and(|m| m != id),
                            Message::DropOnFolder(id),
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
                    // Pinned first, then a group per top section, then those in none. A note in a
                    // section inside one is in its top one's group (with its own section as a chip).
                    let order: Vec<i64> = self.children_of(None).iter().map(|s| s.folder.id).collect();
                    let home = self.default_folder_id();
                    for (folder, notes) in group_by(others, |n| self.base_folder(n.folder_id).or(home), &order) {
                        // The heading and its notes are one place to drop: a note dropped there
                        // moves to the folder.
                        let label = match folder.and_then(|id| self.folders.iter().find(|s| s.folder.id == id)) {
                            Some(s) => format!("Move to {}", s.folder.title),
                            None => "Take out of its section".to_string(),
                        };
                        let group = column![self.folder_header(folder, notes.len()), block(&notes)].spacing(8);
                        page = page.push(
                            drop_zone(group, self.dragging.is_some(), Message::DropInFolder(folder))
                                .hint(DropHint::new(label)),
                        );
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

    /// The folders in the folder whose page this is, when they are shown as sections (and it has
    /// any): their notes are then listed under them.
    pub(super) fn sections_here(&self) -> Option<Vec<&FolderSummary>> {
        match self.scope {
            Scope::Folder(id) if self.group_by_section => {
                let v = self.children_of(Some(id));
                (!v.is_empty()).then_some(v)
            }
            _ => None,
        }
    }

    /// The folders in a folder, as folders to open: an entry each with its icon, name, what it
    /// is for and how many notes it holds, which you can press and hold to move, and drop a note
    /// or a folder on. `None` when it has none.
    fn subfolder_entries(&self, id: i64) -> Option<Element<'_, Message>> {
        let subs = self.children_of(Some(id));
        if subs.is_empty() {
            return None;
        }
        // Cards in a grid, as the playlists on weave's home are: a picture (the section's icon in
        // its colour), its name and what is in it.
        let cards: Vec<Element<'_, Message>> = subs
            .into_iter()
            .map(|s| {
                let f = &s.folder;
                let what = if s.subfolders > 0 {
                    format!("{} · {} section{}", notes_word(s.total), s.subfolders, if s.subfolders == 1 { "" } else { "s" })
                } else {
                    notes_word(s.total)
                };
                let card = ItemCard::new(f.title.clone())
                    .subtitle(what)
                    .picture(icons::tinted(
                        icons::named(f.icon.as_deref()),
                        28.0,
                        Some(self.folder_color(f)),
                    ))
                    .density(organiser_ui::iad::config::Density::Compact)
                    .on_press(Message::SetScope(Scope::Folder(f.id)));
                // A thin border, so a section stands out from the notes around it.
                let card = container(card).style(|theme: &organiser_ui::iad::Theme| {
                    iced::widget::container::Style {
                        border: iced::Border {
                            color: iced::Color {
                                a: 0.22,
                                ..theme.semantic.window.fg
                            },
                            width: 1.0,
                            radius: theme.metrics.radius_m.into(),
                        },
                        ..Default::default()
                    }
                });
                let source = drag_source(Element::from(card), Message::FolderDragStart(f.id));
                let active = self.dragging.is_some() || self.dragging_folder.is_some_and(|m| m != f.id);
                Element::from(
                    drop_zone(source, active, Message::DropOnFolder(f.id)).hint(
                        DropHint::new(format!(
                            "Move into {}",
                            organiser_ui::primitives::text::ellipsize(&f.title, 24)
                        ))
                        .shrink(),
                    ),
                )
            })
            .collect();
        Some(FlowGrid::new(cards).max_width(220.0).spacing(8.0).into())
    }

    /// Whether the views of every note (All, Favorites, Archive, Trash) are in groups, one per
    /// top section: the option is on and there are sections.
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
                        "Group by section",
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

    /// A note that can be picked up, to carry it to a folder of the sidebar or of the page.
    fn draggable<'a>(&self, _note: &Note, e: Element<'a, Message>) -> Element<'a, Message> {
        drag_source(e, Message::DragStart(_note.id)).into()
    }

    /// The top of a folder's page: where it is (the folders it is in, to go up to), its icon in its
    /// colour, its name over what it is for, how many notes, a button for its settings and, beside
    /// it, a menu to show the folders in it as sections and to add one.
    fn folder_page_header(&self, id: i64) -> Element<'_, Message> {
        let Some(s) = self.folder_summary(id) else {
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
            caption(if s.subfolders > 0 {
                format!("{} · {} section{}", notes_word(s.notes), s.subfolders, if s.subfolders == 1 { "" } else { "s" })
            } else {
                notes_word(s.notes)
            }),
            tooltip(
                icon_button(Icon::ListAdd).on_press(Message::NewInFolder(id)),
                "New note in this section",
                TooltipPosition::Bottom,
            ),
            tooltip(
                icon_button(Icon::Edit).on_press(Message::OpenFolderSettings(id)),
                "Section settings",
                TooltipPosition::Bottom,
            ),
            menu_button(
                Icon::ViewMore,
                Menu::new("Section")
                    .section([
                        MenuItem::switch(
                            "Show all notes",
                            self.show_all_notes,
                            Message::ShowAllNotes(!self.show_all_notes),
                        ),
                        MenuItem::switch(
                            "Group by section",
                            self.group_by_section,
                            Message::GroupBySection(!self.group_by_section),
                        ),
                    ])
                    .section([MenuItem::action("New section in here", Message::NewSubfolder(id))]),
            )
            .tooltip("More"),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        let mut col = column![].spacing(2);
        // Where it is: each section it is in, to go up to (the arrow is in the header bar).
        let path = self.folder_path(id);
        if path.len() > 1 {
            let mut crumbs = row![].spacing(2).align_y(Alignment::Center);
            for (i, a) in path[..path.len() - 1].iter().enumerate() {
                if i > 0 {
                    crumbs = crumbs.push(caption("›"));
                }
                crumbs = crumbs.push(
                    iced::widget::button(caption(organiser_ui::primitives::text::ellipsize(&a.folder.title, 20)))
                        .padding([2, 6])
                        .style(organiser_ui::iad::styles::button::flat)
                        .on_press(Message::SetScope(Scope::Folder(a.folder.id))),
                );
            }
            col = col.push(crumbs.wrap());
        }
        col = col.push(head);
        container(col).padding([4, 4]).into()
    }

    /// The heading of a folder shown as a section: a fold arrow, its name over what it is for
    /// (small), how many notes, a button to edit it and one to add a note to it. Press and hold
    /// the name to carry the folder to another.
    fn subfolder_header<'a>(&'a self, s: &'a FolderSummary, shown: usize) -> Element<'a, Message> {
        let f = &s.folder;
        let id = f.id;
        let folded = self.folded_sections.contains(&id);
        let mut name = column![text(f.title.clone()).size(15)].spacing(1);
        if let Some(line) = f.notes.lines().map(str::trim).find(|l| !l.is_empty()) {
            name = name.push(
                text(organiser_ui::primitives::text::ellipsize(line, 110))
                    .size(11)
                    .style(organiser_ui::iad::styles::text::dim),
            );
        }
        // The actions are a tray at the end, flush against the row's edge: add, edit, fold.
        let tray = row![
            tooltip(
                icon_button(Icon::ListAdd).on_press(Message::NewInFolder(id)),
                "Add a note to this section",
                TooltipPosition::Bottom,
            ),
            tooltip(
                icon_button(Icon::Edit).on_press(Message::OpenFolderSettings(id)),
                "Edit section",
                TooltipPosition::Bottom,
            ),
            tooltip(
                icon_button(if folded { Icon::GoNext } else { Icon::GoDown })
                    .on_press(Message::FoldSubfolder(id)),
                if folded { "Show the notes" } else { "Hide the notes" },
                TooltipPosition::Bottom,
            ),
        ]
        .spacing(0)
        .align_y(Alignment::Center);
        // Only the name opens the section: a pointer over it and nothing else (no hover). Press
        // and hold it to carry the section.
        let name = iced::widget::mouse_area(name)
            .interaction(iced::mouse::Interaction::Pointer)
            .on_release(Message::SetScope(Scope::Folder(id)));
        let name = drag_source(name, Message::FolderDragStart(id)).ghost_padding([4, 10]);
        row![
            icons::tinted(icons::named(f.icon.as_deref()), 18.0, Some(self.folder_color(f))),
            name,
            space().width(Length::Fill),
            caption(if s.subfolders > 0 {
                format!("{shown} · {} section{}", s.subfolders, if s.subfolders == 1 { "" } else { "s" })
            } else {
                shown.to_string()
            }),
            tray,
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .padding(iced::Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 10.0 })
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
                Some((Message::OpenFolderSettings(s.folder.id), "Section settings")),
            ),
            None => group_header(None, UNASSIGNED, None, Some(count), None),
        }
    }

    /// Where a note is filed, as a chip, when the notes are not in sections that say so.
    fn folder_chip_of<'a>(&'a self, n: &'a Note) -> Option<Element<'a, Message>> {
        let f = self.folder_of(n)?;
        // One level only. On a section's page that is not grouped, the section of this one the
        // note is in (nothing for a note directly in it); in the groups of All and the like, the
        // section it is in when that is inside the group's; ungrouped, its top section.
        let shown = match self.scope {
            Scope::Folder(here) => {
                // One level below the group: grouped, a note in a section of a section of the page
                // names that deeper one; not grouped, the section of the page it is in.
                let path = self.folder_path(f.id);
                let at = path.iter().position(|s| s.folder.id == here)?;
                let want = at + if self.group_by_section { 2 } else { 1 };
                path.get(want)?.folder.clone()
            }
            _ if self.grouped_by_folder() => {
                if f.parent.is_none() {
                    return None;
                }
                f.clone()
            }
            _ => self.folder_path(f.id).first()?.folder.clone(),
        };
        if shown.is_default {
            return None;
        }
        Some(icons::folder_chip(&shown.title, Some(self.folder_color(&shown)), shown.icon.as_deref()))
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
        self.draggable(n, card(body, self.note_tint(n), selected, Message::Select(n.id)))
    }
}
