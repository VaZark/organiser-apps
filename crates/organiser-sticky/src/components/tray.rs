//! Sticky Notes' tray icon: a StatusNotifierItem, which KDE Plasma and the other desktops that
//! have a tray show. It is the way back to Sticky when its windows are closed or minimised.
//!
//! It runs on a thread of its own and has no Qt in it. What the person does with it (a click, a
//! menu entry) comes back as a [`TrayEvent`] on a channel, which the app reads on its timer; the
//! tray never calls into the UI itself.

use std::sync::mpsc::{Receiver, Sender, channel};

use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::StandardItem;

use crate::STICKY_ID;
use crate::components::store::in_flatpak;

/// What the person asked of the tray icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    /// Show the main window ("All Notes"), from a click on the icon or its first menu entry.
    ShowManager,
    /// Bring every sticky on the desktop to the front.
    BringStickies,
    /// Quit Sticky Notes.
    Quit,
}

impl TrayEvent {
    /// The name QML reads: `manager`, `stickies` or `quit`.
    pub fn name(self) -> &'static str {
        match self {
            Self::ShowManager => "manager",
            Self::BringStickies => "stickies",
            Self::Quit => "quit",
        }
    }
}

struct StickyTray {
    events: Sender<TrayEvent>,
}

impl StickyTray {
    fn send(&self, event: TrayEvent) {
        // The app has gone when nobody listens: nothing to tell.
        let _ = self.events.send(event);
    }

    fn entry(label: &str, icon: &str, event: TrayEvent) -> ksni::MenuItem<Self> {
        StandardItem {
            label: label.into(),
            icon_name: icon.into(),
            activate: Box::new(move |t: &mut Self| t.send(event)),
            ..Default::default()
        }
        .into()
    }
}

impl ksni::Tray for StickyTray {
    fn id(&self) -> String {
        STICKY_ID.into()
    }

    fn title(&self) -> String {
        "Sticky Notes".into()
    }

    fn icon_name(&self) -> String {
        STICKY_ID.into()
    }

    /// A click on the icon.
    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayEvent::ShowManager);
    }

    /// No dividers: three entries, one after the other.
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            Self::entry("All Notes", "view-list-details", TrayEvent::ShowManager),
            Self::entry(
                "Bring Stickies Forward",
                "window-pin",
                TrayEvent::BringStickies,
            ),
            Self::entry("Quit", "application-exit", TrayEvent::Quit),
        ]
    }
}

/// The running tray icon. It goes away when this is dropped.
pub struct Tray {
    events: Receiver<TrayEvent>,
    _handle: Handle<StickyTray>,
}

impl Tray {
    /// Put the icon in the tray. An error (no session bus, no tray on this desktop) only means there
    /// is no icon: the app works without it.
    pub fn spawn() -> Result<Self, String> {
        let (tx, events) = channel();
        let handle = StickyTray { events: tx }
            // Inside a Flatpak the app may not own the `StatusNotifierItem-PID-N` name.
            .disable_dbus_name(in_flatpak())
            .spawn()
            .map_err(|e| format!("no tray icon: {e}"))?;
        Ok(Self {
            events,
            _handle: handle,
        })
    }

    /// The oldest thing asked of the icon that has not been read yet.
    pub fn next(&self) -> Option<TrayEvent> {
        self.events.try_recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_have_the_names_qml_reads() {
        assert_eq!(TrayEvent::ShowManager.name(), "manager");
        assert_eq!(TrayEvent::BringStickies.name(), "stickies");
        assert_eq!(TrayEvent::Quit.name(), "quit");
    }

    #[test]
    fn a_click_and_each_menu_entry_become_events() {
        use ksni::Tray as _;
        let (tx, rx) = channel();
        let mut tray = StickyTray { events: tx };
        tray.activate(0, 0);
        assert_eq!(rx.try_recv(), Ok(TrayEvent::ShowManager));
        // The menu: no separators, and each entry sends its own event.
        let items = tray.menu();
        assert_eq!(items.len(), 3);
        let mut sent = Vec::new();
        for item in items {
            match item {
                ksni::MenuItem::Standard(s) => {
                    (s.activate)(&mut tray);
                    sent.push(rx.try_recv().unwrap());
                }
                other => panic!("only plain entries, got {other:?}"),
            }
        }
        assert_eq!(
            sent,
            [
                TrayEvent::ShowManager,
                TrayEvent::BringStickies,
                TrayEvent::Quit
            ]
        );
    }
}
