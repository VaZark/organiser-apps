//! The window: a header whose title opens the lists, the recipes as cards or compact lines, a
//! details pane that slides in from the right, and dialogs (full-screen sheets on a narrow window)
//! for the lists, adding an entry, the filter and a list's settings. There is no sidebar.

mod calendar;
mod details;
mod filter;
mod fridge;
mod grid;
mod parts;
mod prefs;
mod shop;
mod sidebar;

use organiser_ui::Element;
use organiser_ui::components::new_back::new_or_back;
use organiser_ui::components::panel;
use organiser_ui::components::toggle::IconToggle;
use organiser_ui::iad::layouts::anchor::Side;
use organiser_ui::iad::prelude::*;
use organiser_ui::iad::shortcut::Shortcuts;
use organiser_ui::iad::windows::{Chrome, State};
use organiser_ui::widgets::details_dock::details_dock;
use organiser_ui::widgets::responsive_dialog;

use crate::app::{App, Message, Section, SortChoice, View};

/// Below this window width dialogs are full-screen sheets rather than dialogs.
const TWO_PANES: f32 = 900.0;

impl App {
    pub(crate) fn chrome(&self) -> Chrome {
        Chrome::new(State::default())
    }

    pub fn view(&self) -> Element<'_, Message> {
        let radius = self.chrome().radius();
        let body: Element<'_, Message> = responsive(move |size| {
            // The filter, the log and the shopping list open as a sidebar on the right, in a wide
            // window and a narrow one alike. Preferences stays a dialog.
            let side = if self.shop_open {
                Some((self.shop_panel(), Message::CloseShop))
            } else if self.filter_open {
                Some((self.filter_panel(), Message::CloseFilter))
            } else if self.log_open {
                Some((self.log_panel(), Message::CloseLog))
            } else {
                None
            };
            let base = self.shell(size.width);
            let base: Element<'_, Message> = match side {
                Some((content, close)) => Dock::new(
                    base,
                    panel::side(organiser_ui::components::tab_scope::tab_scope(content)),
                )
                .side(Side::Right)
                .size(DockSize::Fixed(size.width.min(420.0)))
                .mode(DockMode::Overlay)
                .scope(DockScope::Window)
                .window_radius(radius)
                .on_outside(close)
                .into(),
                None => base,
            };
            let prefs = self.prefs.open.then(|| {
                (
                    self.prefs_panel(size.width >= TWO_PANES),
                    Message::ClosePrefs,
                )
            });
            responsive_dialog::present(base, prefs, size.width < TWO_PANES, size, radius, false)
        })
        .into();
        let framed = self.chrome().frame(body, Message::Window);
        let framed = Shortcuts::new()
            .bind("Ctrl+F", Message::ToggleSearch)
            .bind("Ctrl+N", Message::New)
            .bind("Ctrl+,", Message::OpenPrefs)
            .bind("Ctrl+Q", Message::Quit)
            .bind("Esc", Message::Escape)
            .scope(framed);
        match &self.toast {
            Some(text) => toast_stack(
                framed,
                vec![Toast::new(text.clone()).on_dismiss(Message::DismissToast)],
            ),
            None => toast_stack(framed, Vec::new()),
        }
    }

    /// The filter button at the end of the search entry, with how many filters are on.
    fn filter_button(&self) -> Element<'_, Message> {
        let n = self.filter_count();
        IconToggle::new(Icon::Filter, n > 0)
            .tooltip(if n > 0 {
                format!("Filters ({n})")
            } else {
                "Filters".to_string()
            })
            .on_toggle(|_| Message::OpenFilter)
            .into()
    }

    /// What the header's action icons do, as one menu for a narrow window: the view, the sort order
    /// (a submenu, one order ticked) and, inside a list, its settings.
    /// The sort order as an icon of the page's actions, with the orders in a menu, one ticked.
    pub(super) fn sort_button(&self) -> Element<'_, Message> {
        let menu = Menu::new("Sort").section(
            SortChoice::ALL
                .iter()
                .map(|c| MenuItem::check(c.to_string(), c.0 == self.sort, Message::Sort(*c))),
        );
        menu_button(Icon::Sort, menu)
            .tooltip("Sort")
            .open_initially(self.sort_open)
            .into()
    }

    fn actions_menu(&self) -> Element<'_, Message> {
        let mut menu = Menu::new("Menu").section([MenuItem::check(
            "Compact lines",
            self.compact,
            Message::Compact(!self.compact),
        )]);
        menu = menu.section([MenuItem::action("Preferences", Message::OpenPrefs).accel("Ctrl+,")]);
        menu_button(Icon::OpenMenu, menu)
            .tooltip("Menu")
            .open_initially(self.menu_open)
            .open_on_page(self.menu_page)
            .into()
    }

    /// A wide window has a sidebar of lists; a narrow one has none, and its title (a button) opens
    /// the lists as a dialog instead.
    fn shell(&self, width: f32) -> Element<'_, Message> {
        let collapsed = split_collapsed(width);
        let radius = self.chrome().radius();
        // The sidebar takes a quarter of the window, from 180 to 280.
        let room = if collapsed {
            width
        } else {
            width - (width * 0.25).clamp(180.0, 280.0)
        };

        let mut head = header_bar().chrome(&self.chrome().edges(collapsed, true), Message::Window);
        head = if collapsed && self.details.is_none() {
            // A narrow window has no sidebar: the places are a dropdown in the header.
            head.title_widget(self.section_switcher())
        } else {
            head.title(self.heading())
        };
        // "+" makes what the page holds (a recipe, a logged meal), or is back while a recipe is
        // open: the same button in every app. The fridge adds inline, so it has none.
        if self.details.is_some() {
            head = head.pack_start(new_or_back(true, "", Message::New, Message::CloseDetails));
        } else {
            match self.view {
                View::Fridge => {}
                View::Calendar => {
                    head = head.pack_start(new_or_back(
                        false,
                        "Log a meal (Ctrl+N)",
                        Message::OpenLog(None, None),
                        Message::CloseDetails,
                    ));
                }
                _ => {
                    head = head.pack_start(new_or_back(
                        false,
                        "New recipe (Ctrl+N)",
                        Message::New,
                        Message::CloseDetails,
                    ));
                }
            }
        }
        head = head.pack_end(self.actions_menu());
        let pane = self.details.is_some().then(|| self.details_pane());
        let content = toolbar_view(details_dock(self.content_page(room), pane)).top_bar(head);
        if collapsed {
            return content.into();
        }
        let side_head = header_bar()
            .chrome(&self.chrome().edges(true, false), Message::Window)
            .title("Recipes");
        let side = toolbar_view(self.sidebar()).top_bar(side_head);
        NavigationSplitView::new(side, content)
            .adapt(width)
            .show_sidebar(false)
            .window_radius(radius)
            .into()
    }

    /// The places as a dropdown in the header of a narrow window: the title with a chevron.
    fn section_switcher(&self) -> Element<'_, Message> {
        dropdown(Section::ALL, Some(Section::of(self.view)), Message::Go).into()
    }

    fn content_page(&self, room: f32) -> Element<'_, Message> {
        match self.view {
            View::Calendar => self.calendar_page(),
            View::Fridge => self.fridge_page(),
            _ => self.grid_page(room),
        }
    }

    pub(crate) fn heading(&self) -> String {
        match self.view {
            View::All => "Recipes".into(),
            View::Calendar => "Log".into(),
            View::Fridge => "Fridge".into(),
        }
    }
}
