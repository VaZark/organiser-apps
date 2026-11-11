//! The main menu at the end of every app's header, as in GNOME apps: Preferences, and what else
//! an app adds later.

use crate::Element;
use crate::iad::prelude::*;

pub fn primary_menu<'a, M: Clone + 'a>(on_preferences: M) -> Element<'a, M> {
    menu_button(
        Icon::OpenMenu,
        Menu::new("Menu")
            .section([MenuItem::action("Preferences", on_preferences).accel("Ctrl+,")]),
    )
    .tooltip("Main menu")
    .into()
}
