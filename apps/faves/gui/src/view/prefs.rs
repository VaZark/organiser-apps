//! Faves' own page of the Preferences sheet.

use organiser_ui::Element;
use organiser_ui::iad::prelude::*;

use crate::app::{App, Message};

impl App {
    pub(super) fn prefs_panel(&self, wide: bool) -> Element<'_, Message> {
        let compact = EntryRow::switch(
            "Compact lines",
            self.prefs.settings.faves.compact,
            Message::PrefCompact,
        )
        .description("Show one line per entry instead of cards.")
        .flat(true);
        let page =
            iced::widget::column![compact, self.sticky.row(Message::PrefStickyLink)].spacing(12);
        self.prefs.view(
            wide,
            "Faves",
            page.into(),
            None,
            self.tag_manager.view().map(Message::Tags),
            self.tag_manager.in_editor(),
            Message::Pref,
            Message::ClosePrefs,
        )
    }
}
