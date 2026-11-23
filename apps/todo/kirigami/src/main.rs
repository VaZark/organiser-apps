//! `todo-kirigami`: the Todo app in Kirigami. All behaviour is in `todo-schema` and the engine
//! (see `core.rs`); `backend.rs` exposes it to QML and `qml/` draws it.

mod backend;
mod core;

fn main() {
    if organiser_qt::print_desktop_entry_requested(&todo_schema::META, "todo-kirigami") {
        return;
    }
    std::process::exit(organiser_qt::run(
        &todo_schema::META,
        &std::env::var("TODO_QML").unwrap_or("qrc:/qt/qml/org/organiser/todo/qml/Main.qml".into()),
    ));
}
