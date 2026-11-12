//! "Link to sticky notes": the Preferences row every app shows, and the state behind it.
//!
//! Linking registers the app's database with the Sticky app (see `organiser-sticky`). Until the
//! Sticky app is installed the row says so and cannot be switched; the per-item sticky toggle in
//! an app shows only while it is linked.

use iced::widget::{container, text};
use organiser_core::sticky::StickySpec;
use organiser_sticky::{discover, store};

use crate::Element;
use crate::iad::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StickyLink {
    /// The Sticky app is installed.
    pub available: bool,
    /// This app's database is linked to it.
    pub linked: bool,
}

impl StickyLink {
    pub fn load(spec: &StickySpec) -> Self {
        // Tell Sticky Notes about this app, so it can be switched on from either side.
        let _ = store::publish(spec);
        Self {
            available: discover::available(),
            linked: store::is_linked(spec),
        }
    }

    /// Re-read both (call when Preferences opens: Sticky may have been installed or the link
    /// changed from the Sticky app since).
    pub fn refresh(&mut self, spec: &StickySpec) {
        *self = Self::load(spec);
    }

    /// Link or unlink; the error says why it did not work.
    pub fn set(&mut self, spec: &StickySpec, on: bool) -> Result<(), String> {
        if on && !self.available {
            return Err("Sticky Notes is not installed".into());
        }
        store::set_linked(spec, on)?;
        self.linked = on;
        Ok(())
    }

    /// Whether items can be made stickies from this app.
    pub fn usable(&self) -> bool {
        self.available && self.linked
    }

    /// Start Sticky so a newly flagged item shows at once.
    pub fn open_sticky(&self) -> Result<(), String> {
        discover::launch()
    }

    /// The Preferences row.
    pub fn row<'a, M: Clone + 'a>(&self, on_toggle: impl Fn(bool) -> M + 'a) -> Element<'a, M> {
        if self.available {
            EntryRow::switch("Link to sticky notes", self.linked, on_toggle)
                .description("Let the Sticky Notes app show this app's stickies on the desktop.")
                .flat(true)
                .into()
        } else {
            EntryRow::new(
                "Link to sticky notes",
                EntryInput::Custom(container(text("Not installed")).into()),
            )
            .description("Install the Sticky Notes app to show stickies on the desktop.")
            .flat(true)
            .into()
        }
    }
}
