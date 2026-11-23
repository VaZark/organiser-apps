//! `recipes-kirigami`: the Recipes app in Kirigami. All behaviour is in `recipes-schema` and the
//! engine (see `core.rs`); `backend.rs` exposes it to QML and `qml/` draws it.

mod backend;
mod core;

fn main() {
    if organiser_qt::print_desktop_entry_requested(&recipes_schema::META, "recipes-kirigami") {
        return;
    }
    std::process::exit(organiser_qt::run(
        &recipes_schema::META,
        &std::env::var("RECIPES_QML")
            .unwrap_or("qrc:/qt/qml/org/organiser/recipes/qml/Main.qml".into()),
    ));
}
