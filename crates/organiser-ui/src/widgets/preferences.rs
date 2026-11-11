//! The Preferences sheet: pages of settings in the same dialog (a full-screen sheet on a narrow
//! window) as the filters. The shared settings file (`organiser_core::settings`) holds the values;
//! this keeps them, saves every change as it is made, and draws two pages every app has:
//!
//! * **Appearance**: the theme;
//! * **Tags**: the colours new tags take, and the way into the tags dialog.
//!
//! The third page, **This app**, is the app's own: it passes its rows in, and saves what they
//! change with [`Preferences::save`]. Explanations are the rows' descriptions, beneath them.

use std::collections::HashMap;

use iced::widget::{column, row};
use iced::{Alignment, Length};
use organiser_core::settings::{Settings, ThemeChoice};

use crate::Element;
use crate::components::dot::{color_from_hex, dot};
use crate::components::row::selectable;
use crate::iad::prelude::*;
use crate::iad::primitives::color_picker::Hsva;
use crate::widgets::filter_sheet::filter_sheet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Appearance,
    Tags,
    /// A page the app adds of its own kind (Todo's statuses), after Tags.
    Extra,
    /// The app's own settings.
    App,
}

#[derive(Debug, Clone)]
pub enum PrefMsg {
    /// Choose a page where the list stays beside it (a wide window).
    Page(Page),
    /// Go into a page from the list of pages (a narrow window, where the list is the first page).
    Open(Page),
    /// Back from a page to the list of pages (narrow).
    Back,
    Theme(ThemeChoice),
    PaletteColor(usize, Hsva),
    PaletteText(usize, String),
    PaletteAdd,
    PaletteRemove(usize),
    PaletteReset,
    /// Open (true) or leave (false) the palette's editor.
    PaletteEdit(bool),
}

/// What a message changed that the app has to act on now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Changed {
    Theme(ThemeChoice),
    Palette,
    Nothing,
}

pub struct Preferences {
    pub open: bool,
    pub page: Page,
    pub settings: Settings,
    /// Why the settings file could not be read, if it could not: shown on the page.
    pub problem: Option<String>,
    /// The palette's editor is showing, in place of the tags page's lists.
    pub palette_editing: bool,
    /// Narrow window: a page is open (otherwise the list of pages is showing, as the first page).
    pub page_open: bool,
    /// What each palette entry shows, so typing is not interrupted.
    texts: HashMap<usize, String>,
}

fn hex(h: Hsva) -> String {
    let [r, g, b] = h.to_rgb8();
    format!("#{r:02x}{g:02x}{b:02x}")
}

impl Preferences {
    /// Read the shared file (or its defaults).
    pub fn load() -> Self {
        let loaded = Settings::load();
        Self {
            open: false,
            page: Page::Appearance,
            settings: loaded.settings,
            problem: loaded.problem,
            palette_editing: false,
            page_open: false,
            texts: HashMap::new(),
        }
    }

    /// Show the sheet from its start: on a narrow window, the list of pages.
    pub fn open_first_page(&mut self) {
        self.open = true;
        self.page_open = false;
    }

    /// Leave a page for the list of pages, if one is open (a narrow window's Back). Returns whether
    /// it was, for an app's Escape.
    pub fn leave_page(&mut self) -> bool {
        std::mem::take(&mut self.page_open)
    }

    /// Write the settings. Call it after changing `settings` from the app's own page.
    pub fn save(&mut self) -> Result<(), String> {
        self.settings.save()
    }

    /// Apply a message, save, and say what the app has to do about it.
    pub fn update(&mut self, msg: PrefMsg) -> Result<Changed, String> {
        let mut changed = Changed::Nothing;
        match msg {
            PrefMsg::Page(p) => {
                self.page = p;
                return Ok(Changed::Nothing);
            }
            PrefMsg::Open(p) => {
                self.page = p;
                self.page_open = true;
                return Ok(Changed::Nothing);
            }
            PrefMsg::Back => {
                self.page_open = false;
                return Ok(Changed::Nothing);
            }
            PrefMsg::Theme(t) => {
                self.settings.appearance.theme = t;
                changed = Changed::Theme(t);
            }
            PrefMsg::PaletteColor(i, h) => {
                if let Some(c) = self.settings.tags.palette.get_mut(i) {
                    *c = hex(h);
                    self.texts.insert(i, hex(h));
                    changed = Changed::Palette;
                }
            }
            PrefMsg::PaletteText(i, text) => {
                let parsed = Hsva::parse(&text).map(hex);
                self.texts.insert(i, text);
                match (parsed, self.settings.tags.palette.get_mut(i)) {
                    (Some(c), Some(slot)) => {
                        *slot = c;
                        changed = Changed::Palette;
                    }
                    // Half-typed: wait for more.
                    _ => return Ok(Changed::Nothing),
                }
            }
            PrefMsg::PaletteEdit(on) => {
                self.palette_editing = on;
                self.texts.clear();
                return Ok(Changed::Nothing);
            }
            PrefMsg::PaletteAdd => {
                self.settings.tags.palette.push("#9aa0a6".into());
                changed = Changed::Palette;
            }
            PrefMsg::PaletteRemove(i) => {
                // The palette is never empty: a new tag has to get a colour from somewhere.
                if self.settings.tags.palette.len() > 1 && i < self.settings.tags.palette.len() {
                    self.settings.tags.palette.remove(i);
                    self.texts.clear();
                    changed = Changed::Palette;
                }
            }
            PrefMsg::PaletteReset => {
                self.settings.tags = Default::default();
                self.texts.clear();
                changed = Changed::Palette;
            }
        }
        self.save().map(|()| changed)
    }

    /// The sheet. `wide` is whether the window has room for the page list beside the page.
    /// `extra` is an optional page of the app's own kind, after Tags (a title and its content).
    /// `app_title` and `app_page` are the app's settings page. `tags` is the tag manager's view, shown
    /// on top of the Tags page; `tags_editing` is whether it is showing a tag's editor, when the
    /// palette below is out of the way.
    #[allow(clippy::too_many_arguments)]
    pub fn view<'a, M: Clone + 'a>(
        &'a self,
        wide: bool,
        app_title: &'a str,
        app_page: Element<'a, M>,
        extra: Option<(&'a str, Element<'a, M>)>,
        tags: Element<'a, M>,
        tags_editing: bool,
        wrap: impl Fn(PrefMsg) -> M + Clone + 'a,
        on_close: M,
    ) -> Element<'a, M> {
        let (extra_title, extra_page) = match extra {
            Some((t, e)) => (Some(t), Some(e)),
            None => (None, None),
        };
        let page = match self.page {
            Page::Appearance => self.appearance(wrap.clone()),
            Page::Tags => self.tags(wrap.clone(), tags, tags_editing),
            // An app with no extra page never shows it: fall back to its own.
            Page::Extra => extra_page.unwrap_or(app_page),
            Page::App => app_page,
        };
        let page: Element<'a, M> = match &self.problem {
            Some(p) => column![tag(p.clone(), Role::Warning), page]
                .spacing(12)
                .into(),
            None => page,
        };
        let mut pages = vec![
            (Page::Appearance, "Appearance", Icon::Moon),
            (Page::Tags, "Tags", Icon::Tags),
        ];
        if let Some(t) = extra_title {
            pages.push((Page::Extra, t, Icon::Palette));
        }
        pages.push((Page::App, app_title, Icon::Settings));
        // A row of the page list: the page's icon and name, the whole row a target. Flush with the
        // sheet's edge, so the list lines up with the title above it.
        let entry = |p: Page, label: &str, icon: Icon, chevron: bool, msg: M| -> Element<'a, M> {
            let mut r = row![icon.view(18.0), text(label.to_string()).width(Length::Fill)]
                .spacing(12)
                .align_y(Alignment::Center)
                .padding([10, 12]);
            if chevron {
                r = r.push(Icon::GoNext.view(14.0));
            }
            selectable(r, !chevron && self.page == p, msg)
        };
        let body: Element<'a, M> = if wide {
            let items = pages
                .iter()
                .map(|(p, label, icon)| entry(*p, label, *icon, false, wrap(PrefMsg::Page(*p))));
            row![
                container(column(items).spacing(2)).width(190),
                container(page).width(Length::Fill),
            ]
            .spacing(16)
            .align_y(Alignment::Start)
            .into()
        } else if !self.page_open {
            // The first page: what there is to choose from. Tapping one goes into it.
            let items = pages
                .iter()
                .map(|(p, label, icon)| entry(*p, label, *icon, true, wrap(PrefMsg::Open(*p))));
            boxed_list(items)
        } else {
            let title = pages
                .iter()
                .find(|(p, _, _)| *p == self.page)
                .map_or("", |(_, t, _)| *t);
            // A tag's or the palette's editor has its own back, to its list.
            let nested = tags_editing || self.palette_editing;
            if nested {
                page
            } else {
                let head = row![
                    tooltip(
                        icon_button(Icon::GoPrevious).on_press(wrap(PrefMsg::Back)),
                        "Back",
                        TooltipPosition::Bottom
                    ),
                    heading(title.to_string()),
                ]
                .spacing(10)
                .align_y(Alignment::Center);
                column![head, page].spacing(16).into()
            }
        };
        filter_sheet("Preferences", None, body, on_close, None, None)
    }

    fn appearance<'a, M: Clone + 'a>(&'a self, wrap: impl Fn(PrefMsg) -> M + 'a) -> Element<'a, M> {
        EntryRow::select(
            "Theme",
            &ThemeChoice::ALL,
            Some(self.settings.appearance.theme),
            move |t| wrap(PrefMsg::Theme(t)),
        )
        .description("Follow the desktop, or always use light or dark. Applies to every app.")
        .flat(true)
        .into()
    }

    fn tags<'a, M: Clone + 'a>(
        &'a self,
        wrap: impl Fn(PrefMsg) -> M + Clone + 'a,
        tags: Element<'a, M>,
        editing: bool,
    ) -> Element<'a, M> {
        let palette = &self.settings.tags.palette;
        let swatch =
            |c: &str| color_from_hex(c).unwrap_or(iced::Color::from_rgb8(0x9a, 0xa0, 0xa6));

        // Out of the way: just the colours, and a button to change them.
        if !self.palette_editing {
            let dots: Vec<Element<'a, M>> = palette.iter().map(|c| dot(swatch(c), 22.0)).collect();
            let summary = row![
                container(row(dots).spacing(8).wrap()).width(Length::Fill),
                flat_button(row![Icon::Edit.view(16.0), text("Edit")].spacing(6))
                    .on_press(wrap(PrefMsg::PaletteEdit(true))),
            ]
            .spacing(12)
            .align_y(Alignment::Center);
            let palette_row = EntryRow::new(
                "Colours for new tags",
                EntryInput::Custom(summary.into()),
            )
            .description(
                "A new tag takes one of these by its name, so the same tag always looks the same.",
            )
            .flat(true);
            // Editing a tag takes the whole page; the palette is for when the list shows.
            if editing {
                return tags;
            }
            return column![tags, palette_row].spacing(24).into();
        }

        // The editor: every colour on its own line, to change or delete, and add / reset.
        let last = palette.len() <= 1;
        let mut list = column![].spacing(8);
        for (i, c) in palette.iter().enumerate() {
            let color = swatch(c);
            let hsva = Hsva::from_rgb8(
                (color.r * 255.0).round() as u8,
                (color.g * 255.0).round() as u8,
                (color.b * 255.0).round() as u8,
            );
            let typed = self.texts.get(&i).map_or(c.as_str(), String::as_str);
            let (w1, w2, w3) = (wrap.clone(), wrap.clone(), wrap.clone());
            let picker = ColorInput::new(
                hsva,
                typed,
                move |t| w1(PrefMsg::PaletteText(i, t)),
                move |h| w2(PrefMsg::PaletteColor(i, h)),
            )
            .trigger(container(dot(color, 22.0)).padding(4));
            let mut remove = icon_button(Icon::Delete);
            if !last {
                remove = remove.on_press(w3(PrefMsg::PaletteRemove(i)));
            }
            list = list.push(
                row![picker, caption(c.clone()).width(Length::Fill), remove]
                    .spacing(10)
                    .align_y(Alignment::Center),
            );
        }
        let actions = row![
            flat_button(row![Icon::ListAdd.view(16.0), text("Add a colour")].spacing(6))
                .on_press(wrap(PrefMsg::PaletteAdd)),
            flat_button(text("Reset to defaults")).on_press(wrap(PrefMsg::PaletteReset)),
        ]
        .spacing(8)
        .wrap();
        let head = row![
            tooltip(
                icon_button(Icon::GoPrevious).on_press(wrap(PrefMsg::PaletteEdit(false))),
                "Back to the tags",
                TooltipPosition::Bottom
            ),
            text("Colours for new tags"),
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        let colours = EntryRow::new(
            "Colours",
            EntryInput::Custom(column![actions, list].spacing(12).into()),
        )
        .description(
            "A new tag takes one of these by its name. Tap a colour to change it; the last one stays.",
        )
        .flat(true);
        column![head, colours].spacing(16).into()
    }
}

#[cfg(test)]
mod tests {
    use organiser_core::settings::DEFAULT_PALETTE;
    use organiser_engine::TAG_PALETTE;

    use super::*;

    /// A preferences value that does not read or write the user's real file.
    fn prefs() -> Preferences {
        Preferences {
            open: true,
            page: Page::Appearance,
            settings: Settings::default(),
            problem: None,
            palette_editing: false,
            page_open: false,
            texts: HashMap::new(),
        }
    }

    /// Tests that touch `XDG_CONFIG_HOME` take turns: it is one value for the whole process.
    static ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// `update` saves to the real settings path; point it at a scratch directory first, and keep
    /// the returned guard for as long as the test uses it.
    fn scratch(name: &str) -> std::sync::MutexGuard<'static, ()> {
        let guard = ENV.lock().unwrap_or_else(|e| e.into_inner());
        let dir =
            std::env::temp_dir().join(format!("organiser-ui-prefs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // SAFETY: only these tests change the variable, and they hold `ENV` while they do.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &dir) };
        guard
    }

    #[test]
    fn the_engine_and_the_settings_start_from_the_same_palette() {
        assert_eq!(TAG_PALETTE, DEFAULT_PALETTE);
    }

    #[test]
    fn choosing_a_page_changes_nothing_else_and_writes_nothing() {
        let mut p = prefs();
        assert_eq!(
            p.update(PrefMsg::Page(Page::Tags)).unwrap(),
            Changed::Nothing
        );
        assert_eq!(p.page, Page::Tags);
    }

    #[test]
    fn the_palette_editor_opens_and_closes_without_touching_the_settings() {
        let mut p = prefs();
        let before = p.settings.clone();
        assert_eq!(
            p.update(PrefMsg::PaletteEdit(true)).unwrap(),
            Changed::Nothing
        );
        assert!(p.palette_editing);
        p.update(PrefMsg::PaletteEdit(false)).unwrap();
        assert!(!p.palette_editing);
        assert_eq!(p.settings, before);
    }

    #[test]
    fn on_a_narrow_window_the_list_is_the_first_page_and_back_returns_to_it() {
        let mut p = prefs();
        p.page_open = true;
        p.open_first_page();
        assert!(
            p.open && !p.page_open,
            "the sheet starts on the list of pages"
        );
        p.update(PrefMsg::Open(Page::Tags)).unwrap();
        assert_eq!((p.page, p.page_open), (Page::Tags, true));
        assert!(p.leave_page(), "Escape or Back leaves the page");
        assert!(
            !p.page_open && !p.leave_page(),
            "and there is nothing more to leave"
        );
        p.update(PrefMsg::Open(Page::App)).unwrap();
        p.update(PrefMsg::Back).unwrap();
        assert!(!p.page_open);
    }

    #[test]
    fn choosing_a_page_beside_the_list_does_not_navigate() {
        let mut p = prefs();
        p.update(PrefMsg::Page(Page::Tags)).unwrap();
        assert_eq!(
            (p.page, p.page_open),
            (Page::Tags, false),
            "a wide window keeps its list"
        );
    }

    #[test]
    fn the_theme_choice_is_kept_and_reported() {
        let _env = scratch("theme");
        let mut p = prefs();
        let changed = p.update(PrefMsg::Theme(ThemeChoice::Dark)).unwrap();
        assert_eq!(changed, Changed::Theme(ThemeChoice::Dark));
        assert_eq!(p.settings.appearance.theme, ThemeChoice::Dark);
        assert_eq!(
            Settings::load().settings.appearance.theme,
            ThemeChoice::Dark,
            "and saved"
        );
    }

    #[test]
    fn the_palette_can_be_edited_but_never_emptied() {
        let _env = scratch("palette");
        let mut p = prefs();
        let n = p.settings.tags.palette.len();
        p.update(PrefMsg::PaletteAdd).unwrap();
        assert_eq!(p.settings.tags.palette.len(), n + 1);
        p.update(PrefMsg::PaletteColor(0, Hsva::from_rgb8(1, 2, 3)))
            .unwrap();
        assert_eq!(p.settings.tags.palette[0], "#010203");
        p.update(PrefMsg::PaletteText(0, "#0a0b0c".into())).unwrap();
        assert_eq!(p.settings.tags.palette[0], "#0a0b0c");
        p.update(PrefMsg::PaletteText(0, "#0a".into())).unwrap();
        assert_eq!(p.settings.tags.palette[0], "#0a0b0c", "half-typed waits");
        for _ in 0..30 {
            p.update(PrefMsg::PaletteRemove(0)).unwrap();
        }
        assert_eq!(p.settings.tags.palette.len(), 1, "the last colour stays");
        p.update(PrefMsg::PaletteReset).unwrap();
        assert_eq!(p.settings.tags.palette, DEFAULT_PALETTE);
    }
}
