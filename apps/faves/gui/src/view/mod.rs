//! The window: a header whose title opens the lists, the entries as cards or compact lines, a
//! details pane that slides in from the right, and dialogs (full-screen sheets on a narrow window)
//! for the lists, adding an entry, the filter and a list's settings. There is no sidebar.

mod details;
mod filter;
mod grid;
mod lists;
mod prefs;
mod settings;
mod sidebar;

use organiser_ui::Element;
use organiser_ui::components::new_back::new_or_back;
use organiser_ui::components::page_switcher::title_button;
use organiser_ui::components::search;
use organiser_ui::components::toggle::IconToggle;
use organiser_ui::iad::prelude::*;
use organiser_ui::iad::shortcut::Shortcuts;
use organiser_ui::iad::windows::{Chrome, State};
use organiser_ui::widgets::details_dock::details_dock;
use organiser_ui::widgets::responsive_dialog;

use crate::app::{App, Message, SortChoice, View};

/// Below this window width dialogs are full-screen sheets rather than dialogs.
const TWO_PANES: f32 = 900.0;

impl App {
    pub(crate) fn chrome(&self) -> Chrome {
        Chrome::new(State::default())
    }

    pub fn view(&self) -> Element<'_, Message> {
        let radius = self.chrome().radius();
        let body: Element<'_, Message> = responsive(move |size| {
            // One dialog at a time, the one opened last on top.
            let lists_on_top = self.lists_open && !self.filter_open && !self.prefs.open;
            let modal = if self.prefs.open {
                Some((
                    self.prefs_panel(size.width >= TWO_PANES),
                    Message::ClosePrefs,
                ))
            } else if self.filter_open {
                Some((self.filter_panel(), Message::CloseFilter))
            } else if self.lists_open {
                Some((self.lists_panel(), Message::CloseLists))
            } else if self.settings_open {
                Some((self.settings_panel(), Message::CloseSettings))
            } else {
                None
            };
            responsive_dialog::present(
                self.shell(size.width),
                modal,
                size.width < TWO_PANES,
                size,
                radius,
                // The lists open as a full-screen page that fades in rather than slides up.
                lists_on_top,
            )
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
    fn actions_menu(&self) -> Element<'_, Message> {
        let mut menu = Menu::new("Menu").section([MenuItem::check(
            "Compact lines",
            self.compact,
            Message::Compact(!self.compact),
        )]);
        // "Sort ›" slides to a page with the four orders, one ticked, and a back row.
        let sort = Menu::new("Sort").section(
            SortChoice::ALL
                .iter()
                .map(|c| MenuItem::check(c.to_string(), c.0 == self.sort, Message::Sort(*c))),
        );
        menu = menu.section([MenuItem::submenu("Sort", sort)]);
        if self.current().is_some() {
            menu = menu.section([MenuItem::action("List settings", Message::OpenSettings)]);
        }
        menu = menu.section([MenuItem::action("Preferences", Message::OpenPrefs).accel("Ctrl+,")]);
        menu_button(Icon::OpenMenu, menu)
            .tooltip("Menu")
            .open_initially(self.menu_open)
            .open_on_page(self.menu_page)
            .into()
    }

    /// The title, as a button that opens the lists.
    fn lists_button(&self) -> Element<'_, Message> {
        title_button(self.heading(), "Switch list", Message::OpenLists)
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
        if self.search.is_open() && search::takes_over(width) {
            // In a narrow window an open search takes over the whole header, window buttons
            // included, so there is room to see what is typed.
            head = head
                .show_start_title_buttons(false)
                .show_end_title_buttons(false)
                .pack_start(search::close_button(Message::ToggleSearch))
                .title_widget(search::title(
                    self.search.text(),
                    "Search",
                    Message::SearchInput,
                    Some(self.filter_button()),
                ));
        } else {
            head = if self.search.is_open() {
                head.title_widget(search::title(
                    self.search.text(),
                    "Search",
                    Message::SearchInput,
                    Some(self.filter_button()),
                ))
            } else if collapsed {
                head.title_widget(self.lists_button())
            } else {
                head.title(self.heading())
            };
            // New entry, or back while the edit page is open: the same button in every app.
            head = head.pack_start(new_or_back(
                self.details.is_some(),
                "New entry (Ctrl+N)",
                Message::New,
                Message::CloseDetails,
            ));
            // The menu (the view, the sort, the list's settings, Preferences) and search are all
            // the header keeps at the end.
            if self.search.is_open() {
                head = head.pack_start(search::balance(2, 1, room));
            }
            head = head
                .pack_end(self.actions_menu())
                .pack_end(search::button(Message::ToggleSearch));
        }

        let pane = self.details.is_some().then(|| self.details_pane());
        let content = toolbar_view(details_dock(self.grid_page(room), pane)).top_bar(head);
        if collapsed {
            return content.into();
        }
        let side_head = header_bar()
            .chrome(&self.chrome().edges(true, false), Message::Window)
            .title("Faves");
        let side = toolbar_view(self.sidebar()).top_bar(side_head);
        NavigationSplitView::new(side, content)
            .adapt(width)
            .show_sidebar(false)
            .window_radius(radius)
            .into()
    }

    pub(crate) fn heading(&self) -> String {
        match self.view {
            View::All => "All".into(),
            View::Favorites => "Favorites".into(),
            View::List(_) => self
                .current()
                .map(|s| organiser_ui::primitives::text::ellipsize(&s.list.title, 30))
                .unwrap_or_default(),
        }
    }
}
