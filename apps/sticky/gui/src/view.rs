//! The two kinds of window: a sticky (a coloured paper with a title and text) and the manager.

use iced::widget::{button, column, container, mouse_area, row, text_editor, text_input};
use iced::{Alignment, Background, Border, Color, Length, window};
use organiser_ui::Element;
use organiser_ui::components::card::card;
use organiser_ui::components::chip::{chip, color_of};
use organiser_ui::components::dot::{color_from_hex, dot};
use organiser_ui::components::empty::empty;
use organiser_ui::components::primary_menu::primary_menu;
use organiser_ui::components::row::selectable;
use organiser_ui::iad::Theme;
use organiser_ui::iad::prelude::*;
use organiser_ui::iad::primitives::color_picker::Hsva;
use organiser_ui::iad::shortcut::Shortcuts;
use organiser_ui::iad::windows::{Chrome, State};
use organiser_ui::widgets::filter_sheet::filter_sheet;
use organiser_ui::widgets::responsive_dialog;

use crate::app::{App, Message, PrefsPage, Sticky};
use crate::color;

/// Below this window width the preferences are a full-screen sheet.
const TWO_PANES: f32 = 700.0;
const CARD_WIDTH: f32 = 240.0;
const GAP: f32 = 12.0;
const PAGE_PADDING: f32 = 16.0;

impl App {
    pub fn view(&self, id: window::Id) -> Element<'_, Message> {
        match self.stickies.iter().find(|s| s.win == Some(id)) {
            Some(s) => sticky(self, s),
            None => self.manager_view(),
        }
    }
}

/// The sheet of a sticky: its paper, with a thin line round it, as rounded as the main window.
/// `a` laid over `b` by `t`, opaque (a translucent line would show the desktop through).
fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::from_rgb(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
    )
}

fn sheet<'a>(content: Element<'a, Message>, paper: Color, line: Color) -> Element<'a, Message> {
    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(paper)),
            border: Border {
                color: line,
                width: 1.0,
                radius: Chrome::new(State::default()).radius().into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn label_owned<'a>(t: String, ink: Color) -> Element<'a, Message> {
    text(t).size(14).color(ink).into()
}

fn sticky<'a>(app: &'a App, s: &'a Sticky) -> Element<'a, Message> {
    let paper = color::paper(s.color.as_deref(), app.default_color());
    let ink = color::ink(paper);
    let faint = Color { a: 0.55, ..ink };
    let Some(win) = s.win else {
        return space().into();
    };
    let theme = organiser_ui::shell::theme(false);

    // A tool of the top strip: an icon in the sticky's ink, a soft wash when hovered.
    let tool = |icon: Icon, tip: &'static str, msg: Message| -> Element<'a, Message> {
        tooltip(
            button(icon.colored(16.0, ink))
                .padding(6)
                .on_press(msg)
                .style(move |_: &Theme, status| button::Style {
                    background: matches!(status, button::Status::Hovered)
                        .then_some(Background::Color(Color { a: 0.12, ..ink })),
                    border: Border {
                        radius: 8.0.into(),
                        ..Border::default()
                    },
                    ..button::Style::default()
                }),
            tip,
            TooltipPosition::Bottom,
        )
    };
    // The "⋮" menu, as in the reference Sticky Notes: every note (Sticky Notes' main window) and
    // the paper colour on a page of its own. Drawn in the sticky's paper and ink.
    let line = mix(paper, ink, 0.25);
    let row_btn = |content: Element<'a, Message>, msg: Message| -> Element<'a, Message> {
        button(content)
            .width(Length::Fill)
            .padding([6, 10])
            .on_press(msg)
            .style(move |_: &Theme, status| button::Style {
                background: matches!(status, button::Status::Hovered)
                    .then_some(Background::Color(Color { a: 0.12, ..ink })),
                text_color: ink,
                border: Border {
                    radius: 8.0.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            })
            .into()
    };
    let label = |t: &'a str| text(t).size(14).color(ink);
    let menu_page: Element<'a, Message> = if s.menu_colours {
        let current = s.color.as_deref();
        // The first five of the palette, one row each: a round swatch and the colour's name.
        let rows = app.settings.sticky.palette.iter().take(5).map(|hex| {
            let on = current.is_some_and(|c| c.eq_ignore_ascii_case(hex));
            let fill = color::paper(Some(hex), app.default_color());
            let swatch = container(space().width(16).height(16)).style(move |_: &Theme| {
                container::Style {
                    background: Some(Background::Color(fill)),
                    border: Border {
                        color: mix(fill, ink, 0.35),
                        width: 1.0,
                        radius: 8.0.into(),
                    },
                    ..container::Style::default()
                }
            });
            let mark: Element<'a, Message> = if on {
                Icon::Check.colored(14.0, ink).into()
            } else {
                space().width(14).into()
            };
            row_btn(
                row![
                    swatch,
                    text(color::name(hex)).size(14).color(ink),
                    space().width(Length::Fill),
                    mark
                ]
                .spacing(10)
                .align_y(Alignment::Center)
                .into(),
                Message::ColorText(win, hex.clone()),
            )
        });
        column![
            row_btn(
                row![Icon::GoPrevious.colored(14.0, ink), label("Colour")]
                    .spacing(8)
                    .align_y(Alignment::Center)
                    .into(),
                Message::MenuColours(win, false)
            ),
            column(rows).spacing(2),
        ]
        .spacing(2)
        .into()
    } else {
        column![
            row_btn(label("All notes").into(), Message::OpenManager(win)),
            row_btn(
                row![
                    label("Colour"),
                    space().width(Length::Fill),
                    Icon::GoNext.colored(14.0, ink)
                ]
                .align_y(Alignment::Center)
                .into(),
                Message::MenuColours(win, true)
            ),
        ]
        .into()
    };
    let surface = move |page: Element<'a, Message>| {
        container(page)
        .width(190)
        .padding(6)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(paper)),
            border: Border {
                color: line,
                width: 1.0,
                radius: 12.0.into(),
            },
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.25),
                offset: iced::Vector::new(0.0, 2.0),
                blur_radius: 10.0,
            },
            ..container::Style::default()
        })
    };
    let menu: Element<'a, Message> = Popover::new(Icon::ViewMore.colored(16.0, ink), surface(menu_page))
        .appearance(PopoverAppearance::Flat)
        .keep_open(true)
        .scrim(false)
        .align(PopoverAlign::End)
        .into();
    // Closed by changing the shape of the view (see `Sticky::menu_flip`).
    let more: Element<'a, Message> = if s.menu_flip {
        container(menu).into()
    } else {
        menu
    };

    // "+": choose which linked app to create in.
    let apps: Vec<Element<'a, Message>> = app
        .links()
        .enabled()
        .map(|l| {
            let name = if l.name.is_empty() {
                l.slug.clone()
            } else {
                l.name.clone()
            };
            row_btn(label_owned(name, ink), Message::New(win, l.app_id.clone()))
        })
        .collect();
    let choices: Element<'a, Message> = if apps.is_empty() {
        container(text("Link an app in Sticky Notes first.").size(13).color(faint))
            .padding([6, 10])
            .into()
    } else {
        column(apps).spacing(2).into()
    };
    let add_menu: Element<'a, Message> = Popover::new(
        Icon::ListAdd.colored(16.0, ink),
        surface(column![
            container(text("New in…").size(12).color(faint)).padding([4, 10]),
            choices
        ].into()),
    )
    .appearance(PopoverAppearance::Flat)
    .keep_open(true)
    .scrim(false)
    .into();
    let add: Element<'a, Message> = if s.menu_flip {
        container(add_menu).into()
    } else {
        add_menu
    };

    // The strip to drag the sticky by, with its tools: colours, edit/read, remove.
    let top = mouse_area(
        row![
            add,
            space().width(Length::Fill),
            more,
            tool(
                if s.editing { Icon::Eye } else { Icon::Edit },
                if s.editing {
                    "Read"
                } else {
                    "Edit the Markdown"
                },
                Message::ToggleEdit(win)
            ),
            tool(
                Icon::WindowClose,
                "Hide from the desktop (it stays in Sticky Notes)",
                Message::Hide(win)
            ),
        ]
        .align_y(Alignment::Center)
        .padding([4, 6]),
    )
    .on_press(Message::Drag(win));

    let title = text_input("Title", &s.title)
        .on_input(move |t| Message::Title(win, t))
        .size(16)
        .padding([2, 12])
        .style(move |_: &Theme, _| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            icon: ink,
            placeholder: faint,
            value: ink,
            selection: Color { a: 0.3, ..ink },
        });

    let body: Element<'a, Message> = if s.editing {
        text_editor(&s.body)
            .placeholder("Write in Markdown…")
            .on_action(move |a| Message::Body(win, a))
            .height(Length::Fill)
            .padding([4, 12])
            .style(move |_: &Theme, _| text_editor::Style {
                background: Background::Color(Color::TRANSPARENT),
                border: Border::default(),
                placeholder: faint,
                value: ink,
                selection: Color { a: 0.3, ..ink },
            })
            .into()
    } else if s.doc.is_empty() {
        container(
            text("Nothing yet. Press the pencil to write.")
                .size(13)
                .color(faint),
        )
        .padding([4, 12])
        .height(Length::Fill)
        .into()
    } else {
        scrolled_window(container(RichText::new(&theme, &s.doc).compact(true)).padding([4, 12]))
            .height(Length::Fill)
            .into()
    };

    // The app the item came from: a chip, not removable. The corner beside it resizes.
    let grip = mouse_area(space().width(18).height(18))
        .on_press(Message::Resize(win))
        .interaction(iced::mouse::Interaction::ResizingDiagonallyDown);
    let source = row![chip(s.app_name.as_str()), space().width(Length::Fill), grip]
        .align_y(Alignment::Center)
        .padding([6, 12]);

    sheet(
        column![top, title, body, source].into(),
        paper,
        mix(paper, ink, 0.25),
    )
}

impl App {
    fn chrome(&self) -> Chrome {
        Chrome::new(State::default())
    }

    /// The main window: every sticky of the linked apps as a card, and the preferences over it.
    fn manager_view(&self) -> Element<'_, Message> {
        let radius = self.chrome().radius();
        let body: Element<'_, Message> = responsive(move |size| {
            let width = size.width;
            let head = header_bar()
                .chrome(&self.chrome(), Message::Window)
                .title("Sticky Notes")
                .pack_end(primary_menu(Message::OpenPrefs));
            let page = toolbar_view(self.cards_page(width)).top_bar(head);
            let modal = self
                .prefs_open
                .then(|| (self.prefs_panel(width >= TWO_PANES), Message::ClosePrefs));
            responsive_dialog::present(page.into(), modal, width < TWO_PANES, size, radius, false)
        })
        .into();
        let framed = self.chrome().frame(body, Message::Window);
        let framed = Shortcuts::new()
            .bind("Ctrl+,", Message::OpenPrefs)
            .bind("Ctrl+Q", Message::Quit)
            .bind("Esc", Message::Escape)
            .scope(framed);
        match &self.toast {
            Some(t) => toast_stack(
                framed,
                vec![Toast::new(t.clone()).on_dismiss(Message::DismissToast)],
            ),
            None => toast_stack(framed, Vec::new()),
        }
    }

    /// All the stickies, in as many columns as fit; a card brings its sticky forward.
    fn cards_page(&self, width: f32) -> Element<'_, Message> {
        if self.stickies.is_empty() {
            let hint = if self.links().links.is_empty() {
                "Link an app in Preferences, then turn an item into a sticky."
            } else {
                "Turn an item of a linked app into a sticky and it appears here."
            };
            return empty(Icon::Note, "No stickies yet", hint);
        }
        let room = (width - 2.0 * PAGE_PADDING + GAP).max(CARD_WIDTH);
        let cols = ((room / (CARD_WIDTH + GAP)) as usize).max(1);
        let theme = self.theme(window::Id::unique());
        let mut lanes: Vec<(usize, Vec<Element<'_, Message>>)> =
            (0..cols).map(|_| (0, Vec::new())).collect();
        for s in &self.stickies {
            let lane = lanes
                .iter_mut()
                .min_by_key(|(load, _)| *load)
                .expect("at least one column");
            lane.0 += 2 + s
                .body
                .text()
                .lines()
                .filter(|l| !l.trim().is_empty())
                .count()
                .min(6);
            lane.1.push(sticky_card(&theme, self.default_color(), s));
        }
        let grid = row(lanes.into_iter().map(|(_, cards)| {
            column(cards)
                .spacing(GAP)
                .width(Length::FillPortion(1))
                .into()
        }))
        .spacing(GAP)
        .align_y(Alignment::Start);
        scrolled_window(container(grid).padding(PAGE_PADDING))
            .height(Length::Fill)
            .into()
    }

    /// Preferences: the apps that offer stickies and whether each is on, the paper colours, and
    /// whether Sticky starts on login.
    fn prefs_panel(&self, wide: bool) -> Element<'_, Message> {
        let mut apps = column![].spacing(12);
        if self.links().links.is_empty() {
            apps = apps.push(caption(
                "No app has offered stickies yet. Open Notes, Todo or Faves once and it will appear here.",
            ));
        }
        for link in &self.links().links {
            let id = link.app_id.clone();
            apps = apps.push(
                EntryRow::switch(link.title(), link.enabled, move |on| {
                    Message::SetLinked(id.clone(), on)
                })
                .description(link.db_path.clone())
                .flat(true),
            );
        }

        let sticky = &self.settings.sticky;
        let swatch =
            |c: &str| color_from_hex(c).unwrap_or(iced::Color::from_rgb8(0xff, 0xf5, 0x9d));
        let hsva = |c: &str| {
            let k = swatch(c);
            Hsva::from_rgb8(
                (k.r * 255.0).round() as u8,
                (k.g * 255.0).round() as u8,
                (k.b * 255.0).round() as u8,
            )
        };
        // The paper of a sticky that has no colour of its own.
        let default = ColorInput::new(
            hsva(&sticky.default_color),
            &self.default_text,
            Message::DefaultText,
            Message::DefaultPick,
        )
        .swatches(sticky.palette.iter())
        .trigger(container(dot(swatch(&sticky.default_color), 22.0)).padding(4));
        let default_row = EntryRow::new(
            "Default colour",
            EntryInput::Custom(
                row![default, caption(sticky.default_color.clone())]
                    .spacing(10)
                    .align_y(Alignment::Center)
                    .into(),
            ),
        )
        .description("The paper of a sticky that has no colour of its own.")
        .flat(true);

        // The palette: every colour on its own line, to change or delete, and add / reset.
        let last = sticky.palette.len() <= 1;
        let mut list = column![].spacing(8);
        for (i, c) in sticky.palette.iter().enumerate() {
            let typed = self
                .palette_texts
                .get(&i)
                .map_or(c.as_str(), String::as_str);
            let picker = ColorInput::new(
                hsva(c),
                typed,
                move |t| Message::PaletteText(i, t),
                move |h| Message::PaletteColor(i, h),
            )
            .trigger(container(dot(swatch(c), 22.0)).padding(4));
            let mut remove = icon_button(Icon::Delete);
            if !last {
                remove = remove.on_press(Message::PaletteRemove(i));
            }
            list = list.push(
                row![picker, caption(c.clone()).width(Length::Fill), remove]
                    .spacing(10)
                    .align_y(Alignment::Center),
            );
        }
        let actions = row![
            flat_button(row![Icon::ListAdd.view(16.0), text("Add a colour")].spacing(6))
                .on_press(Message::PaletteAdd),
            flat_button(text("Reset to defaults")).on_press(Message::PaletteReset),
        ]
        .spacing(8)
        .wrap();
        let palette_row = EntryRow::new(
            "Colours",
            EntryInput::Custom(column![actions, list].spacing(12).into()),
        )
        .description(
            "The choices in a sticky's colour menu. Tap a colour to change it; the last one stays.",
        )
        .flat(true);

        let general: Element<'_, Message> = EntryRow::switch(
            "Start on login",
            self.autostart,
            Message::Autostart,
        )
        .description(
            "Show your stickies when you log in. Sticky Notes ends by itself if there are none.",
        )
        .flat(true)
        .into();
        let colours: Element<'_, Message> = column![default_row, palette_row].spacing(16).into();
        let page = match self.prefs_page {
            PrefsPage::Apps => apps.into(),
            PrefsPage::Colours => colours,
            PrefsPage::General => general,
        };

        // A row of the page list: the page's icon and name, the whole row a target.
        let entry = |p: PrefsPage, chevron: bool, msg: Message| -> Element<'_, Message> {
            let mut r = row![p.icon().view(18.0), text(p.title()).width(Length::Fill)]
                .spacing(12)
                .align_y(Alignment::Center)
                .padding([10, 12]);
            if chevron {
                r = r.push(Icon::GoNext.view(14.0));
            }
            selectable(r, !chevron && self.prefs_page == p, msg)
        };
        let body: Element<'_, Message> = if wide {
            // The list of pages stays beside the page.
            let items = PrefsPage::ALL
                .iter()
                .map(|p| entry(*p, false, Message::PrefsPage(*p)));
            row![
                container(column(items).spacing(2)).width(190),
                container(page).width(Length::Fill),
            ]
            .spacing(16)
            .align_y(Alignment::Start)
            .into()
        } else if !self.prefs_page_open {
            // Narrow: the list is the first page; tapping a row goes into it.
            let items = PrefsPage::ALL
                .iter()
                .map(|p| entry(*p, true, Message::PrefsOpen(*p)));
            boxed_list(items)
        } else {
            let head = row![
                tooltip(
                    icon_button(Icon::GoPrevious).on_press(Message::PrefsBack),
                    "Back",
                    TooltipPosition::Bottom
                ),
                heading(self.prefs_page.title()),
            ]
            .spacing(10)
            .align_y(Alignment::Center);
            column![head, page].spacing(16).into()
        };
        filter_sheet("Preferences", None, body, Message::ClosePrefs, None, None)
    }
}

/// A sticky as a card: its paper as the tint, the title, the start of the text, and its app.
fn sticky_card<'a>(theme: &Theme, default: &str, s: &'a Sticky) -> Element<'a, Message> {
    let title = if s.title.trim().is_empty() {
        "Untitled".to_string()
    } else {
        s.title.clone()
    };
    let mut col = column![text(title).size(15)].spacing(6);
    if !s.doc.is_empty() {
        // The text as it reads, in a box of limited height: a card is a preview. When there is
        // more than fits, an ellipsis says so.
        col = col.push(
            container(
                RichText::new(theme, &s.doc)
                    .compact(true)
                    .max_blocks(CARD_BLOCKS),
            )
            .max_height(CARD_BODY)
            .clip(true),
        );
        let lines = s
            .body
            .text()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count();
        if s.doc.len() > CARD_BLOCKS || lines > CARD_LINES {
            col = col.push(text("…").size(15));
        }
    }
    // The app it came from (a chip, not removable) and whether it is on the desktop: the same
    // height, on one centre line.
    let source = chip_of_height(&s.app_name, FOOT);
    let key = s.key.clone();
    let on = s.show_on_desktop;
    let pin = tooltip(
        button(
            container(Icon::Pin.colored(14.0, theme.semantic.window.fg))
                .center(Length::Fixed(FOOT)),
        )
        .padding(0)
        .width(FOOT)
        .height(FOOT)
        .on_press(Message::Desktop(key, !on))
        .style(move |theme: &Theme, status| {
            let accent = theme.semantic.swatch(Role::Accent).color;
            let hover = matches!(status, button::Status::Hovered);
            button::Style {
                background: Some(Background::Color(Color {
                    a: match (on, hover) {
                        (true, _) => 0.45,
                        (false, true) => 0.2,
                        (false, false) => 0.0,
                    },
                    ..accent
                })),
                border: Border {
                    radius: (FOOT / 2.0).into(),
                    ..Border::default()
                },
                ..button::Style::default()
            }
        }),
        if on {
            "On the desktop: press to keep it only here"
        } else {
            "Only here: press to show it on the desktop"
        },
        TooltipPosition::Bottom,
    );
    col = col.push(
        row![source, space().width(Length::Fill), pin]
            .align_y(Alignment::Center)
            .height(FOOT),
    );
    card(
        col,
        Some(color::paper(s.color.as_deref(), default)),
        false,
        Message::Show(s.key.clone()),
    )
}

/// The tallest the drawn text of a card gets, and how much counts as more than fits.
const CARD_BODY: f32 = 150.0;
const CARD_BLOCKS: usize = 6;
const CARD_LINES: usize = 8;
/// The height of the card's foot: the app chip and the pin are both this tall.
const FOOT: f32 = 22.0;

/// The app's chip, `height` px tall (a pill, as the pin beside it), in the app's own colour.
fn chip_of_height<'a>(name: &str, height: f32) -> Element<'a, Message> {
    container(
        Chip::new(name.to_string())
            .color(color_of(name))
            .size(ChipSize::Medium),
    )
    .height(height)
    .center_y(Length::Fixed(height))
    .into()
}
