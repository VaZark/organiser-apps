//! `notes-kirigami`: the Notes app in Kirigami. All behaviour is in `notes-schema` and the engine
//! (see `core.rs`); `backend.rs` exposes it to QML and `qml/` draws it.

mod backend;
mod core;

fn main() {
    if organiser_qt::print_desktop_entry_requested(&notes_schema::META, "notes-kirigami") {
        return;
    }
    std::process::exit(organiser_qt::run(
        &notes_schema::META,
        &std::env::var("NOTES_QML")
            .unwrap_or("qrc:/qt/qml/org/organiser/notes/qml/Main.qml".into()),
    ));
}
