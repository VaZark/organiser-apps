//! `notes-gui`: the Notes app. All behaviour is in `notes-schema` and the engine; this is state,
//! messages and views.

mod app;
mod view;

use app::App;
use organiser_ui::shell;

fn main() -> iced::Result {
    if std::env::args().nth(1).as_deref() == Some("--print-desktop-entry") {
        print!("{}", shell::linked_desktop_entry(&notes_schema::META, "notes-gui"));
        return Ok(());
    }
    shell::init();
    iced::application(App::boot, App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .subscription(App::subscription)
        .window(shell::window(&notes_schema::META, window_size()))
        .settings(iced::Settings {
            default_text_size: 14.67.into(),
            antialiasing: true,
            ..Default::default()
        })
        .run()
}

/// 1200×720, or `NOTES_GUI_SIZE=WxH` (for screenshots of the narrow layout).
fn window_size() -> iced::Size {
    std::env::var("NOTES_GUI_SIZE")
        .ok()
        .and_then(|v| {
            let (w, h) = v.split_once('x')?;
            Some(iced::Size::new(w.parse().ok()?, h.parse().ok()?))
        })
        .unwrap_or(iced::Size::new(1200.0, 720.0))
}
