//! The window. There are four views of the same tasks, switched from the menu:
//!
//! * the **notes**: every task a note, in a grid of cards (the default);
//! * the **board**: a kanban board, a column per status;
//! * the **list**: the nested task list;
//! * the **quadrants**: the Eisenhower matrix, four boxes each a compact list.
//!
//! All have a sidebar of All, Current tasks and epics, open a task's details beside or over
//! them, and open the filter from the search.

mod board;
mod details;
mod filter;
mod list;
mod notes;
mod prefs;
mod quadrants;
mod sidebar;
mod statuses;

use organiser_ui::Element;
use organiser_ui::components::new_back::new_or_back;
use organiser_ui::components::search;
use organiser_ui::components::toggle::IconToggle;
use organiser_ui::iad::prelude::*;
use organiser_ui::iad::shortcut::Shortcuts;
use organiser_ui::iad::windows::{Chrome, State};
use organiser_ui::widgets::details_dock::{details_dock, details_drawer};
use organiser_ui::widgets::responsive_dialog;
use todo_schema::sort::SortKey;
use todo_schema::views::Scope;

use crate::app::{App, Message, View};

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
            let page = responsive_dialog::present(window, modal, narrow, size, radius, false);
            // The question about an epic's tasks is a small alert over everything.
            present_dialog(page, self.epic_delete_dialog(), Some(Message::EpicDeleteCancel), radius)
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

    /// "Delete this epic?": what becomes of its tasks. `None` unless one is waiting for the answer.
    fn epic_delete_dialog(&self) -> Option<Element<'_, Message>> {
        let id = self.confirm_epic_delete?;
        let summary = self.epics.iter().find(|s| s.epic.id == id)?;
        let n = summary.total;
        Some(
            AlertDialog::new("Delete this epic?")
                .body(format!(
                    "“{}” has {n} task{}. Keep them, without an epic, or delete them with it?",
                    summary.epic.title,
                    if n == 1 { "" } else { "s" }
                ))
                .response("Cancel", ResponseAppearance::Default, Message::EpicDeleteCancel)
                .response("Keep tasks", ResponseAppearance::Suggested, Message::EpicDeleteKeep)
                .response("Delete tasks", ResponseAppearance::Destructive, Message::EpicDeleteAll)
                .into(),
        )
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
            // The drawer leaves the header alone: the list is still there, so no back button.
            self.details.is_some() && !self.in_drawer(window),
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

    /// The view, the order and what is shown, and Preferences: everything the header does not need a
    /// button for. The views and the orders are submenus; what is shown are switches.
    fn actions_menu(&self) -> Element<'_, Message> {
        let views = Menu::new("View")
            .section(
                View::ALL
                    .map(|v| MenuItem::select(v.label(), self.view == v, Message::SetView(v))),
            );
        // The order of the tasks (pinned ones are always first): a submenu, one order ticked.
        let sort = Menu::new("Sort by").section(
            SortKey::ALL
                .map(|k| MenuItem::select(k.label(), self.sort == k, Message::SetSort(k))),
        );
        let mut shows = vec![
            MenuItem::switch(
                "Show closed tasks",
                self.show_closed,
                Message::ShowClosed(!self.show_closed),
            ),
            MenuItem::switch(
                "Show subtasks",
                self.show_subtasks,
                Message::ShowSubtasks(!self.show_subtasks),
            ),
        ];
        if self.view == View::List {
            let current = self.scope == Scope::Current;
            let flat = MenuItem::switch("Flat list", self.is_flat(), Message::Flat(!self.flat));
            shows.push(if current { flat.disabled() } else { flat });
        }
        menu_button(
            Icon::OpenMenu,
            Menu::new("Menu")
                .section([
                    MenuItem::submenu("View", views),
                    MenuItem::submenu("Sort by", sort),
                ])
                .section(shows)
                .section([MenuItem::action("Preferences", Message::OpenPrefs).accel("Ctrl+,")]),
        )
        .tooltip("Menu")
        .open_initially(self.menu_open)
        .into()
    }

    /// `base` with the details of the selected task fading in over all of it.
    ///
    /// On a wide window, with the setting on, they are a drawer beside the list instead.
    fn over_details<'a>(
        &'a self,
        base: impl Into<Element<'a, Message>>,
        width: f32,
    ) -> Element<'a, Message> {
        let pane = self.details.is_some().then(|| self.details_pane());
        if self.in_drawer(width) {
            details_drawer(base, pane, Message::CloseDetails)
        } else {
            details_dock(base, pane)
        }
    }

    /// Whether the details are a drawer: the setting is on and the window is wide enough.
    fn in_drawer(&self, width: f32) -> bool {
        self.prefs.settings.todo.details_in_drawer && width >= TWO_PANES
    }

    // ---- the window: a sidebar of All, current tasks and epics, beside the board or the list ----

    fn shell(&self, width: f32) -> Element<'_, Message> {
        let narrow = split_collapsed(width);
        // A wide window can hide its sidebar too, as a narrow one has none docked.
        let collapsed = narrow || self.sidebar_hidden;
        let radius = self.chrome().radius();
        let side_head = header_bar()
            .chrome(&self.chrome().edges(true, false), Message::Window)
            .title("Todo");
        let side = toolbar_view(self.sidebar()).top_bar(side_head);

        // The sidebar button: it opens the drawer of a narrow window, and hides or shows the
        // docked sidebar of a wide one.
        let start: Option<Element<'_, Message>> = Some(
            tooltip(
                icon_button(Icon::Sidebar).on_press(if narrow {
                    Message::Drawer(true)
                } else {
                    Message::ToggleSidebar
                }),
                "Sidebar",
                TooltipPosition::Bottom,
            )
            .into(),
        );
        // The header spans what the sidebar leaves: a quarter of the window, from 180 to 280.
        let room = if collapsed {
            width
        } else {
            width - (width * 0.25).clamp(180.0, 280.0)
        };
        let head = self.header((collapsed, true), start, room, width);
        // The details cover the content rather than share its row, so it has the whole width.
        let page: Element<'_, Message> = match self.view {
            View::Notes => self.notes_page(room),
            View::Board => self.board_page(width),
            View::Quadrants => self.quadrants_page(room),
            View::List => page(self.list_page(width < STACK_BELOW))
                .maximum_size(1000.0)
                .into(),
        };
        let content = toolbar_view(self.over_details(page, width)).top_bar(head);
        NavigationSplitView::new(side, content)
            .adapt(width)
            .collapsed(collapsed)
            .show_sidebar(self.drawer_open)
            .on_close_drawer(Message::Drawer(false))
            .window_radius(radius)
            .into()
    }

    /// The title of what the window shows.
    pub(crate) fn heading(&self) -> String {
        if self.scope == Scope::All
            && let Some(title) = self.epic_filter.and_then(|id| self.epic_titles.get(&id))
        {
            return title.clone();
        }
        match (self.scope, self.view) {
            (Scope::Current, _) => "Current tasks".into(),
            (Scope::All, View::Notes) => "Notes".into(),
            (Scope::All, View::Board) => "Board".into(),
            (Scope::All, View::List) => "All tasks".into(),
            (Scope::All, View::Quadrants) => "Quadrants".into(),
        }
    }
}
