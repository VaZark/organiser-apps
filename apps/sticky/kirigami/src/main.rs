//! `sticky-kirigami`: Sticky Notes in Kirigami. All behaviour is in `sticky-engine` and `core.rs`;
//! `backend.rs` exposes it to QML and `qml/` draws it.
//!
//! `--background` (used at login and when an app flags an item) shows only the stickies and ends
//! if there are none.

mod backend;
mod core;

fn main() {
    // `%u`: the URI the desktop hands over when something opens `organiser-sticky:`; that scheme
    // is how the other apps find and start Sticky (see `organiser_sticky::discover`).
    if std::env::args().nth(1).as_deref() == Some("--print-desktop-entry") {
        let entry = organiser_qt::desktop_entry(&core::META, "sticky-kirigami %u");
        println!(
            "{}\nMimeType=x-scheme-handler/{};",
            entry.trim_end(),
            organiser_sticky::discover::SCHEME
        );
        return;
    }
    std::process::exit(organiser_qt::run(
        &core::META,
        &std::env::var("STICKY_QML")
            .unwrap_or("qrc:/qt/qml/org/organiser/sticky/qml/Main.qml".into()),
    ));
}
