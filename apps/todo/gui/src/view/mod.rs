//! The window. There are two views of the same tasks, switched from the header:
//!
//! * the **list**: a header over the task list, with no sidebar;
//! * the **board**: a kanban board with a sidebar for All, Closed, epics and tags.
//!
//! Both open a task's details in a pane that slides in from the right, and both open the filter
//! from the search.

mod board;
mod details;
mod filter;
mod list;
mod prefs;
mod sidebar;
mod statuses;

use organiser_ui::Element;
use organiser_ui::components::new_back::new_or_back;
use organiser_ui::components::search;
use organiser_ui::components::toggle::IconToggle;
use organiser_ui::iad::prelude::*;
use organiser_ui::iad::shortcut::Shortcuts;
use organiser_ui::iad::windows::{Chrome, State};
use organiser_ui::widgets::details_dock::details_dock;
use organiser_ui::widgets::responsive_dialog;
use todo_schema::views::Scope;

use crate::app::{App, Message};

/// Below this window width the filter is a full-screen sheet rather than a dialog.
const TWO_PANES: f32 = 900.0;
/// Below this list width, chips go under a task's title instead of beside it.
const STACK_BELOW: f32 = 620.0;

impl App {
    pub(crate) fn chrome(&self) -> Chrome {
        Chrome::new(State::default())
    }

    pub fn view(&self) -> Element<'_, Message> {
        let radius = self.chrome().radius();
        let body: Element<'_, Message> = responsive(move |size| {
            let narrow = size.width < TWO_PANES;
            let modal = if self.prefs.open {
                Some((self.prefs_panel(!narrow), Message::ClosePrefs))
            } else {
                self.filter_open
                    .then(|| (self.filter_panel(), Message::CloseFilter))
            };
            let window = self.shell(size.width);
            responsive_dialog::present(window, modal, narrow, size, radius, false)
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
        let button = IconToggle::new(Icon::Filter, n > 0)
            .tooltip(if n > 0 {
                format!("Filters ({n})")
            } else {
                "Filters".to_string()
            })
            .on_toggle(|_| Message::OpenFilter);
        button.into()
    }

    /// The header of either view. `edges` are the window edges it touches (a board's sidebar has
    /// its own header, so the content's may stop short of the left edge).
    ///
    /// `start` is the button at the header's start (back, or the sidebar's), if any, and `room`
    /// the header's width (`window` is the window's, for the narrow-window search). While searching, the entry is centred in the header by balancing the
    /// start side against the end (when there is room for that).
    fn header<'a>(
        &'a self,
        edges: (bool, bool),
        start: Option<Element<'a, Message>>,
        room: f32,
        window: f32,
    ) -> HeaderBar<'a, Message> {
        let mut head = header_bar().chrome(&self.chrome().edges(edges.0, edges.1), Message::Window);
        // In a narrow window an open search takes over the whole header, window buttons included.
        if self.search.is_open() && search::takes_over(window) {
            return head
                .show_start_title_buttons(false)
                .show_end_title_buttons(false)
                .pack_start(search::close_button(Message::ToggleSearch))
                .title_widget(search::title(
                    self.search.text(),
                    "Search tasks",
                    Message::SearchInput,
                    Some(self.filter_button()),
                ));
        }
        let start_present = start.is_some();
        if let Some(button) = start {
            head = head.pack_start(button);
        }
        // New task: the edit page, empty.
        head = head.pack_start(new_or_back(
            self.details.is_some(),
            "New task (Ctrl+N)",
            Message::New,
            Message::CloseDetails,
        ));
        let start_buttons = usize::from(start_present) + 1;
        // Search and the menu are all that is left at the end: everything else is in the menu.
        let end_buttons = 2;
        if self.search.is_open() {
            head = head.pack_start(search::balance(end_buttons, start_buttons, room));
        }
        head = if self.search.is_open() {
            head.title_widget(search::title(
                self.search.text(),
                "Search tasks",
                Message::SearchInput,
                Some(self.filter_button()),
            ))
        } else {
            head.title(self.heading())
        };
        // From the window controls inwards: the menu, then search.
        head.pack_end(self.actions_menu())
            .pack_end(search::button(Message::ToggleSearch))
    }

    /// The view settings and Preferences: everything the header does not need a button for.
    fn actions_menu(&self) -> Element<'_, Message> {
        let mut view = vec![
            MenuItem::check("Kanban board", self.kanban, Message::Kanban(!self.kanban)),
            MenuItem::check(
                "Show closed tasks",
                self.show_closed,
                Message::ShowClosed(!self.show_closed),
            ),
        ];
        if !self.kanban {
            let current = self.scope == Scope::Current;
            let flat = MenuItem::check("Flat list", self.is_flat(), Message::Flat(!self.flat));
            view.push(if current { flat.disabled() } else { flat });
        }
        menu_button(
            Icon::OpenMenu,
            Menu::new("Menu").section(view).section([MenuItem::action(
                "Preferences",
                Message::OpenPrefs,
            )
            .accel("Ctrl+,")]),
        )
        .tooltip("Menu")
        .open_initially(self.menu_open)
        .into()
    }

    /// `base` with the details of the selected task fading in over all of it.
    fn over_details<'a>(&'a self, base: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
        details_dock(base, self.details.is_some().then(|| self.details_pane()))
    }

    // ---- the window: a sidebar of All, current tasks and epics, beside the board or the list ----

    fn shell(&self, width: f32) -> Element<'_, Message> {
        let collapsed = split_collapsed(width);
        let radius = self.chrome().radius();
        let side_head = header_bar()
            .chrome(&self.chrome().edges(true, false), Message::Window)
            .title("Todo");
        let side = toolbar_view(self.sidebar()).top_bar(side_head);

        let start: Option<Element<'_, Message>> = collapsed.then(|| {
            icon_button(Icon::Sidebar)
                .on_press(Message::Drawer(true))
                .into()
        });
        // The header spans what the sidebar leaves: a quarter of the window, from 180 to 280.
        let room = if collapsed {
            width
        } else {
            width - (width * 0.25).clamp(180.0, 280.0)
        };
        let head = self.header((collapsed, true), start, room, width);
        // The details cover the content rather than share its row, so it has the whole width.
        let page: Element<'_, Message> = if self.kanban {
            self.board_page(width)
        } else {
            page(self.list_page(width < STACK_BELOW))
                .maximum_size(820.0)
                .into()
        };
        let content = toolbar_view(self.over_details(page)).top_bar(head);
        NavigationSplitView::new(side, content)
            .adapt(width)
            .show_sidebar(self.drawer_open)
            .on_close_drawer(Message::Drawer(false))
            .window_radius(radius)
            .into()
    }

    /// The title of what the window shows.
    pub(crate) fn heading(&self) -> String {
        match (self.scope, self.kanban) {
            (Scope::Current, _) => "Current tasks".into(),
            (Scope::All, true) => "Board".into(),
            (Scope::All, false) => "All tasks".into(),
        }
    }
}
