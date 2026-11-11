//! The window: a sidebar (Notes, Archive, Trash) beside a Keep-style grid of cards.
//! A note's details slide in over the grid.

mod details;
mod filter;
mod grid;
mod prefs;
mod sidebar;

use notes_schema::views::Scope;
use organiser_ui::Element;
use organiser_ui::components::new_back::new_or_back;
use organiser_ui::components::primary_menu::primary_menu;
use organiser_ui::components::search;
use organiser_ui::components::toggle::IconToggle;
use organiser_ui::iad::prelude::*;
use organiser_ui::iad::shortcut::Shortcuts;
use organiser_ui::iad::windows::{Chrome, State};
use organiser_ui::widgets::details_dock::details_dock;
use organiser_ui::widgets::responsive_dialog;

use crate::app::{App, Message};

/// Below this window width a dialog is a full-screen sheet.
const TWO_PANES: f32 = 900.0;

impl App {
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

    pub(crate) fn chrome(&self) -> Chrome {
        Chrome::new(State::default())
    }

    pub fn view(&self) -> Element<'_, Message> {
        let radius = self.chrome().radius();
        let body: Element<'_, Message> = responsive(move |size| {
            let width = size.width;
            let collapsed = split_collapsed(width);
            let side_head = header_bar()
                .chrome(&self.chrome().edges(true, false), Message::Window)
                .title("Notes");
            let side = toolbar_view(self.sidebar()).top_bar(side_head);

            let start: Option<Element<'_, Message>> = collapsed.then(|| {
                icon_button(Icon::Sidebar)
                    .on_press(Message::Drawer(true))
                    .into()
            });
            let room = if collapsed {
                width
            } else {
                width - (width * 0.25).clamp(180.0, 280.0)
            };
            let head = self.header((collapsed, true), start, room, width);
            let page = details_dock(
                self.grid_page(width),
                self.details.is_some().then(|| self.details_pane()),
            );
            let content = toolbar_view(page).top_bar(head);
            let window: Element<'_, Message> = NavigationSplitView::new(side, content)
                .adapt(width)
                .show_sidebar(self.drawer_open)
                .on_close_drawer(Message::Drawer(false))
                .window_radius(radius)
                .into();
            // One dialog at a time.
            let modal = if self.prefs.open {
                Some((self.prefs_panel(width >= TWO_PANES), Message::ClosePrefs))
            } else if self.filter_open {
                Some((self.filter_panel(), Message::CloseFilter))
            } else {
                None
            };
            responsive_dialog::present(window, modal, width < TWO_PANES, size, radius, false)
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

    /// `edges` are the window edges the header touches; `start` the sidebar button of a narrow
    /// window; `room` the header's width and `window` the window's.
    fn header<'a>(
        &'a self,
        edges: (bool, bool),
        start: Option<Element<'a, Message>>,
        room: f32,
        window: f32,
    ) -> HeaderBar<'a, Message> {
        let start_present = start.is_some();
        let mut head = header_bar().chrome(&self.chrome().edges(edges.0, edges.1), Message::Window);
        let searching = || {
            search::title(
                self.search.text(),
                "Search notes",
                Message::SearchInput,
                Some(self.filter_button()),
            )
        };
        // In a narrow window an open search takes over the whole header, window buttons included.
        if self.search.is_open() && search::takes_over(window) {
            return head
                .show_start_title_buttons(false)
                .show_end_title_buttons(false)
                .pack_start(search::close_button(Message::ToggleSearch))
                .title_widget(searching());
        }
        if let Some(button) = start {
            head = head.pack_start(button);
        }
        // New note: the edit page, empty. Notes are never made without a title and the rest in view.
        head = head.pack_start(new_or_back(
            self.details.is_some(),
            "New note (Ctrl+N)",
            Message::New,
            Message::CloseDetails,
        ));
        let start_buttons = usize::from(start_present) + 1;
        if self.search.is_open() {
            head = head.pack_start(search::balance(2, start_buttons, room));
            head = head.title_widget(searching());
        } else {
            head = head.title(self.heading());
        }
        // The menu and search are all the header keeps at the end.
        head.pack_end(primary_menu(Message::OpenPrefs))
            .pack_end(search::button(Message::ToggleSearch))
    }

    /// The title of what the window shows.
    pub(crate) fn heading(&self) -> String {
        match self.scope {
            Scope::Notes => "Notes".into(),
            Scope::Archive => "Archive".into(),
            Scope::Trash => "Trash".into(),
        }
    }
}
