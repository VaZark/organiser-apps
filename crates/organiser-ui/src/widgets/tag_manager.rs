//! The tags: every tag in its colour, indented under the group it sits in. Tapping a tag opens its
//! editor (in the same sheet or page): a colour, the group it belongs to, delete. Tags can be
//! created from the entry on top. There is no dragging: a list you tap works with a finger as well
//! as a pointer, and what a tag is nested under is something to read, not to aim at.
//!
//! It keeps its own state and messages, so an app only embeds [`TagManager`], wraps [`TagMsg`] in
//! one of its messages, and reloads its tags afterwards. The tag engine has no rename and orders
//! siblings by name, so there is neither here.

use std::collections::{HashMap, HashSet};

use iced::widget::{button, column, row};
use iced::{Alignment, Background, Border, Length};
use organiser_engine::{ChildPolicy, Db, TAG_PALETTE, Tag};

use crate::Element;
use crate::components::action::action_button;
use crate::components::chip::{chip, color_of};
use crate::components::dot::{color_from_hex, dot};
use crate::components::row::selectable;
use crate::iad::Theme;
use crate::iad::prelude::*;
use crate::iad::primitives::color_picker::Hsva;
use crate::widgets::tag_tree;

#[derive(Debug, Clone)]
pub enum TagMsg {
    /// Open the editor of a tag, or (`None`) go back to the list.
    Edit(Option<String>),
    /// A colour picked in the picker or from a swatch.
    Color(String, Hsva),
    /// Text typed in the picker's colour entry.
    ColorText(String, String),
    /// Put a tag under a group, or (`None`) back at the top.
    Parent(String, Option<String>),
    Delete(String),
    /// Open the page that makes a new tag.
    NewStart,
    /// The name typed on that page.
    NewText(String),
    /// A colour for the new tag, as `#rrggbb`.
    NewColor(String),
    /// The group the new tag goes under.
    NewParent(Option<String>),
    /// Create the tag from the page.
    New,
}

/// A group to pick in the dropdown: a tag, or none.
#[derive(Debug, Clone, PartialEq)]
pub struct Group(pub Option<String>);

impl std::fmt::Display for Group {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_deref().unwrap_or("No group"))
    }
}

/// What the list shows of a tag.
struct Row {
    name: String,
    depth: usize,
}

#[derive(Default)]
pub struct TagManager {
    pub open: bool,
    rows: Vec<Row>,
    /// Each tag's parent and colour.
    parents: HashMap<String, Option<String>>,
    colors: HashMap<String, Hsva>,
    /// Each colour as `#rrggbb`: what the picker's entry shows until something is typed.
    codes: HashMap<String, String>,
    /// The colours on offer in the editor, beside the free choice.
    palette: Vec<String>,
    /// The tag being edited, with where it may go: no group, or any tag that is not itself or below
    /// it (that would be a loop).
    editing: Option<String>,
    choices: Vec<Group>,
    /// What the editor's colour entry shows, so typing is not interrupted.
    texts: HashMap<String, String>,
    new_name: String,
    /// The new-tag page is open.
    creating: bool,
    new_color: Option<String>,
    new_parent: Option<String>,
    /// Every tag, as a group to pick for a new tag.
    all_groups: Vec<Group>,
}

fn hex_of(h: Hsva) -> String {
    let [r, g, b] = h.to_rgb8();
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// `name` and every tag below it.
fn subtree(tags: &[Tag], name: &str) -> HashSet<String> {
    let mut out = HashSet::from([name.to_string()]);
    loop {
        let more: Vec<String> = tags
            .iter()
            .filter(|t| {
                t.parent.as_ref().is_some_and(|p| out.contains(p)) && !out.contains(&t.name)
            })
            .map(|t| t.name.clone())
            .collect();
        if more.is_empty() {
            return out;
        }
        out.extend(more);
    }
}

impl TagManager {
    /// Refresh what the dialog lists from the app's tags, and the colours the editor offers. Call
    /// it after every reload.
    pub fn sync(&mut self, tags: &[Tag], palette: &[&str]) {
        self.rows = tag_tree::ordered(tags)
            .into_iter()
            .map(|(depth, t)| Row {
                name: t.name.clone(),
                depth,
            })
            .collect();
        self.parents = tags
            .iter()
            .map(|t| (t.name.clone(), t.parent.clone()))
            .collect();
        self.colors = tags
            .iter()
            .map(|t| {
                let c = t
                    .color
                    .as_deref()
                    .and_then(crate::primitives::color::parse_hex)
                    .map_or(Hsva::from_rgb8(0x9a, 0xa0, 0xa6), |[r, g, b]| {
                        Hsva::from_rgb8(r, g, b)
                    });
                (t.name.clone(), c)
            })
            .collect();
        self.codes = self
            .colors
            .iter()
            .map(|(n, c)| (n.clone(), hex_of(*c)))
            .collect();
        self.palette = if palette.is_empty() {
            TAG_PALETTE.iter().map(|c| c.to_string()).collect()
        } else {
            palette.iter().map(|c| c.to_string()).collect()
        };
        self.all_groups = std::iter::once(Group(None))
            .chain(
                tag_tree::ordered(tags)
                    .into_iter()
                    .map(|(_, t)| Group(Some(t.name.clone()))),
            )
            .collect();
        // A tag deleted elsewhere leaves nothing to edit.
        if self
            .editing
            .as_ref()
            .is_some_and(|n| !self.parents.contains_key(n))
        {
            self.editing = None;
        }
        self.choices = match &self.editing {
            Some(name) => {
                let below = subtree(tags, name);
                std::iter::once(Group(None))
                    .chain(
                        tag_tree::ordered(tags)
                            .into_iter()
                            .filter(|(_, t)| !below.contains(&t.name))
                            .map(|(_, t)| Group(Some(t.name.clone()))),
                    )
                    .collect()
            }
            None => Vec::new(),
        };
    }

    /// The tag whose editor is showing.
    pub fn editing(&self) -> Option<&str> {
        self.editing.as_deref()
    }

    /// Whether an editor page (of a tag, or of a new one) is showing: the sheet's back button
    /// leaves it first.
    pub fn in_editor(&self) -> bool {
        self.editing.is_some() || self.creating
    }

    /// Leave the editor if it is showing. Returns whether it was, for an app's Escape.
    pub fn leave_editor(&mut self) -> bool {
        let was = self.in_editor();
        self.editing = None;
        self.creating = false;
        was
    }

    /// Apply a message to the database. The error says what was wrong; reload the tags (and call
    /// [`sync`](Self::sync)) after success.
    pub fn update(&mut self, db: &mut Db, msg: TagMsg) -> Result<(), String> {
        let fail = |e: organiser_engine::Error| e.to_string();
        match msg {
            TagMsg::Edit(name) => {
                self.editing = name;
                self.creating = false;
                Ok(())
            }
            TagMsg::NewStart => {
                self.creating = true;
                self.editing = None;
                self.new_name.clear();
                self.new_color = None;
                self.new_parent = None;
                Ok(())
            }
            TagMsg::NewColor(hex) => {
                self.new_color = Some(hex);
                Ok(())
            }
            TagMsg::NewParent(p) => {
                self.new_parent = p;
                Ok(())
            }
            TagMsg::Color(name, h) => {
                self.texts.insert(name.clone(), hex_of(h));
                db.set_tag_color(&name, Some(&hex_of(h)))
                    .map(|_| ())
                    .map_err(fail)
            }
            TagMsg::ColorText(name, text) => {
                let parsed = Hsva::parse(&text).map(hex_of);
                self.texts.insert(name.clone(), text);
                match parsed {
                    Some(hex) => db
                        .set_tag_color(&name, Some(&hex))
                        .map(|_| ())
                        .map_err(fail),
                    // Half-typed: wait for more.
                    None => Ok(()),
                }
            }
            TagMsg::Parent(name, parent) => db
                .set_tag_parent(&name, parent.as_deref())
                .map(|_| ())
                .map_err(fail),
            TagMsg::Delete(name) => {
                self.texts.remove(&name);
                self.editing = None;
                // Its children stay, at the top: deleting a group must not take the tags with it.
                db.delete_tag(&name, ChildPolicy::Detach)
                    .map(|_| ())
                    .map_err(fail)
            }
            TagMsg::NewText(t) => {
                self.new_name = t;
                Ok(())
            }
            TagMsg::New => {
                let name = self
                    .new_name
                    .trim()
                    .trim_start_matches(['#', '@'])
                    .to_string();
                if name.is_empty() {
                    return Ok(());
                }
                let (color, parent) = (self.new_color.take(), self.new_parent.take());
                self.new_name.clear();
                self.creating = false;
                db.create_tag(&name, parent.as_deref()).map_err(fail)?;
                if let Some(c) = color {
                    db.set_tag_color(&name, Some(&c)).map_err(fail)?;
                }
                Ok(())
            }
        }
    }

    /// The list of tags, or the editor of one.
    pub fn view(&self) -> Element<'_, TagMsg> {
        if self.creating {
            return self.creator();
        }
        match &self.editing {
            Some(name) => self.editor(name),
            None => self.list(),
        }
    }

    fn list(&self) -> Element<'_, TagMsg> {
        // The title with a "+" that opens the page for a new tag.
        let new = row![
            column![
                text("Tags").size(15),
                caption("A group is a tag other tags sit under; filtering by it finds them all.")
            ]
            .width(Length::Fill),
            tooltip(
                icon_button(Icon::ListAdd).on_press(TagMsg::NewStart),
                "New tag",
                TooltipPosition::Bottom
            ),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let tags: Element<'_, TagMsg> = if self.rows.is_empty() {
            caption("No tags yet.").into()
        } else {
            let rows = self.rows.iter().map(|r| {
                let color = color_of(&r.name);
                selectable(
                    row![
                        space().width(Length::Fixed(24.0 * r.depth as f32)),
                        dot(color, 16.0),
                        chip(&r.name),
                        space().width(Length::Fill),
                        Icon::Settings.view(16.0),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center)
                    // Tall enough to tap.
                    .padding([10, 12]),
                    false,
                    TagMsg::Edit(Some(r.name.clone())),
                )
            });
            boxed_list(rows)
        };
        column![new, tags].spacing(16).into()
    }

    /// The page for a new tag: its name, colour and group, then Create.
    fn creator(&self) -> Element<'_, TagMsg> {
        let head = row![
            tooltip(
                icon_button(Icon::GoPrevious).on_press(TagMsg::Edit(None)),
                "Back to the tags",
                TooltipPosition::Bottom
            ),
            text("New tag").size(16),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let name = EntryRow::new(
            "Name",
            EntryInput::Text(
                Entry::new("Tag name", self.new_name.as_str())
                    .on_input(TagMsg::NewText)
                    .on_submit(TagMsg::New),
            ),
        )
        .description("Starting a name with # or @ is fine; it is dropped.")
        .flat(true);

        let chosen = self.new_color.as_deref().unwrap_or("");
        let mut swatches: Vec<Element<'_, TagMsg>> = self
            .palette
            .iter()
            .filter_map(|hex| {
                let c = color_from_hex(hex)?;
                Some(swatch(
                    c,
                    hex.eq_ignore_ascii_case(chosen),
                    TagMsg::NewColor(hex.clone()),
                ))
            })
            .collect();
        let current = self
            .new_color
            .as_deref()
            .and_then(Hsva::parse)
            .unwrap_or(Hsva::from_rgb8(0x9a, 0xa0, 0xa6));
        swatches.push(
            ColorInput::new(
                current,
                chosen,
                |t| match Hsva::parse(&t) {
                    Some(h) => TagMsg::NewColor(hex_of(h)),
                    None => TagMsg::NewColor(t),
                },
                |h| TagMsg::NewColor(hex_of(h)),
            )
            .swatches(TAG_PALETTE)
            .trigger(container(Icon::Palette.view(20.0)).padding(8))
            .into(),
        );
        let colour = EntryRow::new(
            "Colour",
            EntryInput::Custom(row(swatches).spacing(4).wrap().into()),
        )
        .description("Optional: pick one, or leave it for an automatic colour.")
        .flat(true);

        let group = EntryRow::select(
            "Group",
            &self.all_groups,
            Some(Group(self.new_parent.clone())),
            |g: Group| TagMsg::NewParent(g.0),
        )
        .description("Optional: put it under a group.")
        .flat(true);

        let create = action_button("Create tag")
            .on_press_maybe((!self.new_name.trim().is_empty()).then_some(TagMsg::New));
        column![head, name, colour, group, create]
            .spacing(16)
            .into()
    }

    fn editor<'a>(&'a self, name: &'a str) -> Element<'a, TagMsg> {
        let current = self
            .colors
            .get(name)
            .copied()
            .unwrap_or(Hsva::from_rgb8(0x9a, 0xa0, 0xa6));
        let current_hex = hex_of(current);
        let typed = self
            .texts
            .get(name)
            .or_else(|| self.codes.get(name))
            .map_or("", String::as_str);

        let head = row![
            tooltip(
                icon_button(Icon::GoPrevious).on_press(TagMsg::Edit(None)),
                "Back to the tags",
                TooltipPosition::Bottom
            ),
            chip(name),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        // The palette as big dots (a thumb can hit them), the chosen one ringed, then the picker
        // for any other colour.
        let mut swatches: Vec<Element<'a, TagMsg>> = self
            .palette
            .iter()
            .filter_map(|hex| {
                let c = color_from_hex(hex)?;
                let [r, g, b] = crate::primitives::color::parse_hex(hex)?;
                Some(swatch(
                    c,
                    hex.eq_ignore_ascii_case(&current_hex),
                    TagMsg::Color(name.to_string(), Hsva::from_rgb8(r, g, b)),
                ))
            })
            .collect();
        let (n1, n2) = (name.to_string(), name.to_string());
        swatches.push(
            ColorInput::new(
                current,
                typed,
                move |t| TagMsg::ColorText(n1.clone(), t),
                move |h| TagMsg::Color(n2.clone(), h),
            )
            .swatches(TAG_PALETTE)
            .trigger(container(Icon::Palette.view(20.0)).padding(8))
            .into(),
        );
        let colour = EntryRow::new(
            "Colour",
            EntryInput::Custom(row(swatches).spacing(4).wrap().into()),
        )
        .description("Pick one, or the palette icon for any other colour.")
        .flat(true);

        let here = self.parents.get(name).cloned().flatten();
        let group = EntryRow::select(
            "Group",
            &self.choices,
            Some(Group(here)),
            move |g: Group| TagMsg::Parent(name.to_string(), g.0),
        )
        .description("Put this tag under a group. Filtering by the group finds this tag too.")
        .flat(true);

        let delete = action_row("Delete tag")
            .subtitle("The tags in it stay, at the top.")
            .suffix(destructive_button(text("Delete")).on_press(TagMsg::Delete(name.to_string())));

        column![head, colour, group, delete].spacing(16).into()
    }
}

/// A colour dot big enough to tap, ringed when it is the chosen one.
fn swatch<'a>(color: iced::Color, chosen: bool, on_press: TagMsg) -> Element<'a, TagMsg> {
    button(dot(color, 26.0))
        .padding(5)
        .on_press(on_press)
        .style(move |theme: &Theme, _| button::Style {
            background: None::<Background>,
            border: Border {
                color: if chosen {
                    theme.semantic.swatch(Role::Accent).color
                } else {
                    iced::Color::TRANSPARENT
                },
                width: 2.0,
                radius: 20.0.into(),
            },
            ..button::Style::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use organiser_engine::{Field, FieldType, Kind, Schema};

    use super::*;

    static SCHEMA: Schema = Schema {
        kinds: &[Kind {
            name: "note",
            fields: &[Field::new("body", FieldType::Markdown)],
        }],
    };

    fn setup() -> (Db, TagManager) {
        (Db::open_in_memory(&SCHEMA).unwrap(), TagManager::default())
    }

    fn add(m: &mut TagManager, db: &mut Db, name: &str) {
        m.update(db, TagMsg::NewText(name.into())).unwrap();
        m.update(db, TagMsg::New).unwrap();
    }

    fn synced(m: &mut TagManager, db: &Db) {
        m.sync(&db.tags().unwrap(), &[]);
    }

    fn tag(db: &Db, name: &str) -> Tag {
        db.tags()
            .unwrap()
            .into_iter()
            .find(|t| t.name == name)
            .unwrap()
    }

    #[test]
    fn the_new_tag_page_makes_a_tag_with_its_colour_and_group_and_back_leaves_it() {
        let (mut db, mut m) = setup();
        add(&mut m, &mut db, "meals");
        m.update(&mut db, TagMsg::NewStart).unwrap();
        assert!(m.in_editor());
        m.update(&mut db, TagMsg::NewText("lunch".into())).unwrap();
        m.update(&mut db, TagMsg::NewColor("#ff0000".into()))
            .unwrap();
        m.update(&mut db, TagMsg::NewParent(Some("meals".into())))
            .unwrap();
        m.update(&mut db, TagMsg::New).unwrap();
        assert!(!m.in_editor(), "back on the list once created");
        let t = tag(&db, "lunch");
        assert_eq!(t.parent.as_deref(), Some("meals"));
        assert_eq!(t.color.as_deref(), Some("#ff0000"));
        // Starting again begins empty; Back leaves without making anything.
        m.update(&mut db, TagMsg::NewStart).unwrap();
        m.update(&mut db, TagMsg::NewText("tmp".into())).unwrap();
        assert!(m.leave_editor());
        assert!(!m.leave_editor());
        assert_eq!(db.tags().unwrap().len(), 2);
    }

    #[test]
    fn a_new_tag_is_created_from_the_typed_name_and_the_entry_clears() {
        let (mut db, mut m) = setup();
        add(&mut m, &mut db, "  #work ");
        assert_eq!(db.tags().unwrap().len(), 1);
        assert_eq!(tag(&db, "work").parent, None);
        assert!(m.new_name.is_empty());
        m.update(&mut db, TagMsg::New).unwrap();
        assert_eq!(db.tags().unwrap().len(), 1, "an empty name makes nothing");
    }

    #[test]
    fn colours_come_from_a_swatch_or_typed_text_and_half_typed_text_waits() {
        let (mut db, mut m) = setup();
        add(&mut m, &mut db, "work");
        m.update(
            &mut db,
            TagMsg::Color("work".into(), Hsva::from_rgb8(255, 0, 0)),
        )
        .unwrap();
        assert_eq!(tag(&db, "work").color.as_deref(), Some("#ff0000"));
        m.update(&mut db, TagMsg::ColorText("work".into(), "#00f".into()))
            .unwrap();
        assert_eq!(tag(&db, "work").color.as_deref(), Some("#0000ff"));
        m.update(&mut db, TagMsg::ColorText("work".into(), "#12".into()))
            .unwrap();
        assert_eq!(
            tag(&db, "work").color.as_deref(),
            Some("#0000ff"),
            "half-typed changes nothing"
        );
    }

    #[test]
    fn a_tag_can_be_put_in_a_group_and_taken_out_but_never_in_a_loop() {
        let (mut db, mut m) = setup();
        for n in ["trips", "lisbon"] {
            add(&mut m, &mut db, n);
        }
        m.update(
            &mut db,
            TagMsg::Parent("lisbon".into(), Some("trips".into())),
        )
        .unwrap();
        assert_eq!(tag(&db, "lisbon").parent.as_deref(), Some("trips"));
        assert!(
            m.update(
                &mut db,
                TagMsg::Parent("trips".into(), Some("lisbon".into()))
            )
            .is_err(),
            "a group under its own member would be a loop"
        );
        m.update(&mut db, TagMsg::Parent("lisbon".into(), None))
            .unwrap();
        assert_eq!(tag(&db, "lisbon").parent, None);
    }

    #[test]
    fn the_list_shows_nesting_as_depth_groups_before_their_members() {
        let (mut db, mut m) = setup();
        for n in ["b", "a", "c", "d"] {
            add(&mut m, &mut db, n);
        }
        m.update(&mut db, TagMsg::Parent("c".into(), Some("a".into())))
            .unwrap();
        m.update(&mut db, TagMsg::Parent("d".into(), Some("c".into())))
            .unwrap();
        synced(&mut m, &db);
        let shown: Vec<(&str, usize)> = m.rows.iter().map(|r| (r.name.as_str(), r.depth)).collect();
        assert_eq!(shown, [("a", 0), ("c", 1), ("d", 2), ("b", 0)]);
    }

    #[test]
    fn the_editor_offers_every_group_except_the_tag_itself_and_what_is_below_it() {
        let (mut db, mut m) = setup();
        for n in ["a", "b", "c", "d"] {
            add(&mut m, &mut db, n);
        }
        m.update(&mut db, TagMsg::Parent("b".into(), Some("a".into())))
            .unwrap();
        m.update(&mut db, TagMsg::Parent("c".into(), Some("b".into())))
            .unwrap();
        m.update(&mut db, TagMsg::Edit(Some("a".into()))).unwrap();
        synced(&mut m, &db);
        let offered = |m: &TagManager| m.choices.iter().map(Group::to_string).collect::<Vec<_>>();
        assert_eq!(offered(&m), ["No group", "d"], "not a, b or c");
        m.update(&mut db, TagMsg::Edit(Some("b".into()))).unwrap();
        synced(&mut m, &db);
        assert_eq!(offered(&m), ["No group", "a", "d"], "not b or c");
        assert_eq!(m.editing(), Some("b"));
        assert!(m.leave_editor());
        assert!(!m.leave_editor(), "already out");
    }

    #[test]
    fn deleting_a_group_keeps_its_tags_at_the_top_and_closes_the_editor() {
        let (mut db, mut m) = setup();
        for n in ["trips", "lisbon"] {
            add(&mut m, &mut db, n);
        }
        m.update(
            &mut db,
            TagMsg::Parent("lisbon".into(), Some("trips".into())),
        )
        .unwrap();
        m.update(&mut db, TagMsg::Edit(Some("trips".into())))
            .unwrap();
        m.update(&mut db, TagMsg::Delete("trips".into())).unwrap();
        assert_eq!(db.tags().unwrap().len(), 1);
        assert_eq!(tag(&db, "lisbon").parent, None);
        assert_eq!(m.editing(), None);
    }

    #[test]
    fn a_tag_deleted_elsewhere_leaves_nothing_to_edit() {
        let (mut db, mut m) = setup();
        add(&mut m, &mut db, "gone");
        m.update(&mut db, TagMsg::Edit(Some("gone".into())))
            .unwrap();
        db.delete_tag("gone", ChildPolicy::Detach).unwrap();
        synced(&mut m, &db);
        assert_eq!(m.editing(), None);
    }

    #[test]
    fn the_editor_offers_the_palette_it_was_given_or_the_default() {
        let (mut db, mut m) = setup();
        add(&mut m, &mut db, "x");
        m.sync(&db.tags().unwrap(), &["#112233", "#445566"]);
        assert_eq!(m.palette, ["#112233", "#445566"]);
        m.sync(&db.tags().unwrap(), &[]);
        assert_eq!(m.palette.len(), TAG_PALETTE.len());
    }
}
