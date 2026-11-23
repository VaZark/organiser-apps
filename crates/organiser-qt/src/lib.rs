//! What every Kirigami frontend shares: starting the Qt application for an [`AppMeta`], the
//! `--print-desktop-entry` flag, and the QML components in `qml/` (each app's `build.rs` copies them into its own
//! module).
//!
//! Layering: this crate knows Qt, never an app's schema. Apps own their bridge objects.

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};
use organiser_core::AppMeta;

/// The `.desktop` file text of an app, for `--print-desktop-entry` and the install script.
pub fn desktop_entry(meta: &AppMeta, exec: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName={}\nComment={}\nExec={}\nIcon={}\nCategories={};\nStartupWMClass={}\nTerminal=false\n",
        meta.name,
        meta.comment,
        exec,
        meta.id,
        meta.categories.join(";"),
        meta.id,
    )
}

/// Whether the command line asks for the `.desktop` entry; prints it when it does.
pub fn print_desktop_entry_requested(meta: &AppMeta, exec: &str) -> bool {
    if std::env::args().nth(1).as_deref() == Some("--print-desktop-entry") {
        print!("{}", desktop_entry(meta, exec));
        return true;
    }
    false
}

unsafe extern "C" {
    fn organiser_qt_setup_kde(engine: *mut std::ffi::c_void, domain: *const std::ffi::c_char);
}

/// Run the Qt event loop with the QML file at `main_qml` (a `qrc:` URL) as the window. Returns the
/// process exit code.
pub fn run(meta: &AppMeta, main_qml: &str) -> i32 {
    organiser_core::settings::use_app(meta);
    // The KDE look (Breeze through the `org.kde.desktop` style) unless the person chose one.
    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        // SAFETY: called first thing in `main`, before any other thread exists.
        unsafe { std::env::set_var("QT_QUICK_CONTROLS_STYLE", "org.kde.desktop") };
    }
    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();
    if let Some(mut app) = app.as_mut() {
        app.as_mut().set_application_name(&QString::from(meta.slug));
        app.as_mut()
            .set_organization_name(&QString::from("organiser"));
        QGuiApplication::set_desktop_file_name(&QString::from(meta.id));
    }
    let domain = std::ffi::CString::new(meta.slug).expect("slug has no NUL");
    if let Some(mut engine) = engine.as_mut() {
        // SAFETY: the pointer is the live engine; the shim only registers the KI18n context on it.
        unsafe {
            organiser_qt_setup_kde(
                std::ptr::from_mut(engine.as_mut().get_unchecked_mut()).cast(),
                domain.as_ptr(),
            );
        }
        engine.load(&QUrl::from(main_qml));
    }
    match app.as_mut() {
        Some(app) => app.exec(),
        None => 1,
    }
}
