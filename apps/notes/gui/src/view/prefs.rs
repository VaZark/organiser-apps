//! Notes' own page of the Preferences sheet.

use organiser_ui::Element;
use organiser_ui::iad::prelude::*;

use crate::app::{App, Message};

/// A number of days in the trash, for the dropdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Days(pub u32);

impl std::fmt::Display for Days {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} days", self.0)
    }
}

const CHOICES: [Days; 5] = [Days(7), Days(14), Days(30), Days(60), Days(90)];

impl App {
    pub(super) fn prefs_panel(&self, wide: bool) -> Element<'_, Message> {
        let trash = EntryRow::select(
            "Days in the trash",
            &CHOICES,
            Some(Days(self.prefs.settings.notes.trash_days)),
            |d: Days| Message::PrefTrashDays(d.0),
        )
        .description("A note is deleted for good after this long in the trash.")
        .flat(true);
        let page =
            iced::widget::column![trash, self.sticky.row(Message::PrefStickyLink)].spacing(12);
        self.prefs.view(
            wide,
            "Notes",
            page.into(),
            None,
            self.tag_manager.view().map(Message::Tags),
            self.tag_manager.in_editor(),
            Message::Pref,
            Message::ClosePrefs,
        )
    }
}
