//! Compiles the C++ shim in `cpp/kde.cpp` against Qt 6 and KDE Frameworks 6 (KI18n).

use std::process::Command;

/// Ask Qt 6's qmake: `$QMAKE` when set (the Flatpak SDK's is plain `qmake`), else `qmake6`.
fn qmake(var: &str) -> String {
    let name = std::env::var("QMAKE").unwrap_or_else(|_| "qmake6".into());
    let out = Command::new(&name)
        .args(["-query", var])
        .output()
        .unwrap_or_else(|e| {
            panic!("{name} not found ({e}): install Qt 6 development files or set QMAKE")
        });
    String::from_utf8(out.stdout)
        .expect("qmake output")
        .trim()
        .to_string()
}

fn main() {
    let headers = qmake("QT_INSTALL_HEADERS");
    let mut build = cc::Build::new();
    build.cpp(true).std("c++17").file("cpp/kde.cpp");
    for dir in [
        headers.clone(),
        format!("{headers}/QtCore"),
        format!("{headers}/QtGui"),
        format!("{headers}/QtQml"),
        format!("{headers}/QtQmlIntegration"),
        format!("{headers}/QtNetwork"),
        "/usr/include/KF6".into(),
        "/usr/include/KF6/KI18n".into(),
        "/usr/include/KF6/KI18nQml".into(),
    ] {
        build.include(dir);
    }
    build.flag_if_supported("-fPIC").compile("organiser_qt_kde");
    println!("cargo:rerun-if-changed=cpp/kde.cpp");
    println!(
        "cargo:rustc-link-search=native={}",
        qmake("QT_INSTALL_LIBS")
    );
    for lib in ["KF6I18n", "KF6I18nQml", "Qt6Qml", "Qt6Core"] {
        println!("cargo:rustc-link-lib={lib}");
    }
}
