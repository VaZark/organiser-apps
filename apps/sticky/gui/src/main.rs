//! `sticky-gui`: Sticky Notes. Shows the flagged items of the linked organiser apps as desktop
//! stickies, and a small "Linked apps" window to link and unlink them.
//!
//! `--background` (used at login and when an app flags an item) shows only the stickies and ends
//! if there are none.

mod app;
mod color;
mod view;

use app::App;
use organiser_core::AppMeta;
use organiser_ui::shell;

pub const META: AppMeta = AppMeta {
    id: organiser_sticky::STICKY_ID,
    slug: "sticky",
    name: "Sticky Notes",
    comment: "Keep notes, tasks and favourites on your desktop",
    categories: &["Office", "Utility"],
};

fn main() -> iced::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--print-desktop-entry") {
        print!("{}", shell::desktop_entry(&META, "sticky-gui"));
        return Ok(());
    }
    let background = args.iter().any(|a| a == "--background");
    shell::init();
    iced::daemon(move || App::boot(background), App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .subscription(App::subscription)
        .settings(iced::Settings {
            default_text_size: 14.67.into(),
            antialiasing: true,
            ..Default::default()
        })
        .run()
}
