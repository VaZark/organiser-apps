//! Startup checks run in separate processes because iad installs global configuration.

use organiser_ui::{iad, shell};
use std::process::Command;

#[test]
fn startup_installs_the_same_theme_for_layout_and_drawing() {
    let root = std::env::temp_dir().join(format!("organiser-theme-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let overlay = root.join("overlay.conf");
    std::fs::write(&overlay, "[metrics]\nradius-window=21\n").unwrap();

    let installed = root.join("installed");
    std::fs::create_dir_all(installed.join("iad/themes")).unwrap();
    std::fs::write(
        installed.join("iad/themes/default.conf"),
        "[metrics]\nradius-window=23\n",
    )
    .unwrap();

    for (case, theme) in [
        ("bundled", None),
        ("installed", None),
        ("overlay", Some(overlay.as_os_str())),
        ("bare", Some(std::ffi::OsStr::new("none"))),
        ("bare-whitespace", Some(std::ffi::OsStr::new(" none "))),
    ] {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "startup_child", "--nocapture"])
            .env("ORGANISER_THEME_TEST", case)
            .env("HOME", &root)
            .env("XDG_CONFIG_HOME", &root)
            .env("XDG_CONFIG_DIRS", &root)
            .env("XDG_DATA_HOME", &root)
            .env("XDG_DATA_DIRS", &root)
            .env_remove("ICED_ADAPTIVE_DESIGN_CONFIG")
            .env_remove("IAD_THEME");
        if case == "installed" {
            command.env("XDG_DATA_HOME", &installed);
        }
        if let Some(theme) = theme {
            command.env("IAD_THEME", theme);
        }
        let output = command.output().unwrap();
        if case.starts_with("bare") {
            assert!(
                output.stderr.is_empty(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        assert!(
            output.status.success(),
            "{case}:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn startup_child() {
    let Ok(case) = std::env::var("ORGANISER_THEME_TEST") else {
        return;
    };
    shell::init();
    let expected_radius = match case.as_str() {
        "bundled" | "bare" | "bare-whitespace" => 15.0,
        "overlay" => 21.0,
        "installed" => 23.0,

        _ => panic!("unknown case {case}"),
    };
    let metrics = iad::Metrics::desktop();
    assert_eq!(metrics.radius_window, expected_radius);
    for dark in [false, true] {
        let theme = shell::theme(dark);
        assert_eq!(theme.metrics.radius_window, metrics.radius_window);
        assert_eq!(theme.metrics.space_m, metrics.space_m);
        assert_eq!(
            theme.scheme,
            if dark {
                iad::Scheme::Dark
            } else {
                iad::Scheme::Light
            }
        );
    }
}
