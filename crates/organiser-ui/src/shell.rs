//! What `main` needs: toolkit config, window settings and the `.desktop` entry.

use organiser_core::AppMeta;

use crate::iad;

/// Load the desktop-wide toolkit config. Call once, first thing in `main`.
pub fn init() {
    iad::config::install(iad::config::Config::load());
}

/// Window settings with client-side decorations and the app id GNOME needs to match the
/// `.desktop` file.
pub fn window(meta: &AppMeta, size: iced::Size) -> iced::window::Settings {
    iced::window::Settings {
        size,
        ..iad::windows::settings_for(meta.id)
    }
}

/// The `.desktop` file text, for `--print-desktop-entry` and the install script.
pub fn desktop_entry(meta: &AppMeta, exec: &str) -> String {
    iad::platform::linux::DesktopEntry {
        app_id: meta.id,
        name: meta.name,
        comment: meta.comment,
        exec,
        icon: meta.id,
        categories: meta.categories,
    }
    .render()
}

/// The toolkit theme for the desktop scheme and the user's override.
pub fn theme(dark: bool) -> iad::Theme {
    if dark {
        iad::Theme::dark()
    } else {
        iad::Theme::light()
    }
}

/// Open a link from a note in the desktop's default handler. Only web and mail links are opened
/// (see [`crate::primitives::link::is_openable`]); the error says why nothing happened.
pub fn open_link(url: &str) -> Result<(), String> {
    if !crate::primitives::link::is_openable(url) {
        return Err(format!("Not opening {url}: only web and mail links open"));
    }
    std::process::Command::new("xdg-open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Cannot open {url}: {e}"))
}
