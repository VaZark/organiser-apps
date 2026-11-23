use std::fs;
use std::path::Path;

use cxx_qt_build::{CxxQtBuilder, QmlModule};

/// Every app shares the QML in `crates/organiser-qt/qml`. cxx-qt serves a file under its path
/// relative to the app, so the shared files are copied into `qml/shared/` (git-ignored) first.
const SHARED: &str = "../../../crates/organiser-qt/qml";

fn main() {
    let mut module = QmlModule::new("org.organiser.notes");
    for f in [
        "Main",
        "NotesPage",
        "NoteCard",
        "NoteRow",
        "NoteDetailsPage",
        "FolderSettingsPage",
        "FilterSheet",
        "SettingsPage",
        "TagsPage",
    ] {
        module = module.qml_file(format!("qml/{f}.qml"));
    }
    println!("cargo:rerun-if-changed={SHARED}");
    fs::create_dir_all("qml/shared").expect("create qml/shared");
    for entry in fs::read_dir(SHARED).expect("read shared QML") {
        let path = entry.expect("shared QML entry").path();
        if path.extension().is_some_and(|e| e == "qml") {
            let name = path.file_name().expect("file name");
            fs::copy(&path, Path::new("qml/shared").join(name)).expect("copy shared QML");
            module = module.qml_file(format!("qml/shared/{}", name.to_string_lossy()));
        }
    }
    CxxQtBuilder::new_qml_module(module)
        .qt_module("Quick")
        .file("src/backend.rs")
        .build();
}
