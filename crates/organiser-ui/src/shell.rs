//! What `main` needs: toolkit config, window settings and the `.desktop` entry.

use organiser_core::AppMeta;

use crate::iad;

const DEFAULT_BASE_THEME: &str = include_str!("../themes/libadwaita.ron");

/// The scheme used until the desktop reports one. The settings portal's `color-scheme` is
/// "prefer-dark" or nothing (`default`, "no preference"), so with nothing reported the apps are
/// light.
pub const DEFAULT_THEME_MODE_PREFERENCE: iad::Scheme = iad::Scheme::Light;

/// [`DEFAULT_THEME_MODE_PREFERENCE`] as the apps' `desktop_dark` flag, before the desktop reports.
pub const fn default_desktop_dark() -> bool {
    matches!(DEFAULT_THEME_MODE_PREFERENCE, iad::Scheme::Dark)
}

/// Load desktop settings and the bundled base with any selected theme overlay.
/// Call once, first thing in `main`. `IAD_THEME=none` skips the overlay,
/// but never disables the bundled base theme.
pub fn init() {
    iad::config::install(iad::config::Config::load_with_base_theme(
        DEFAULT_BASE_THEME,
    ));
}

/// Window settings with client-side decorations and the app id GNOME needs to match the
/// `.desktop` file.
pub fn window(meta: &AppMeta, size: iced::Size) -> iced::window::Settings {
    iced::window::Settings {
        size,
        ..iad::windows::settings_for(meta.id)
    }
}

/// The `.desktop` file text, for `--print-desktop-entry` and the install script.
pub fn desktop_entry(meta: &AppMeta, exec: &str) -> String {
    iad::platform::linux::DesktopEntry {
        app_id: meta.id,
        name: meta.name,
        comment: meta.comment,
        exec,
        icon: meta.id,
        categories: meta.categories,
    }
    .render()
}

/// The URI scheme an app handles so Sticky Notes can open it on its create page from inside a
/// Flatpak sandbox (through the desktop portal): `organiser-<slug>`.
pub fn scheme(meta: &AppMeta) -> String {
    format!("organiser-{}", meta.slug)
}

/// The `.desktop` file text of an app Sticky Notes can open: the command takes `%u`, and the
/// entry declares the app's URI scheme (see [`scheme`]).
pub fn linked_desktop_entry(meta: &AppMeta, bin: &str) -> String {
    let entry = desktop_entry(meta, &format!("{bin} %u"));
    format!(
        "{}\nMimeType=x-scheme-handler/{};\n",
        entry.trim_end(),
        scheme(meta)
    )
}

/// Whether the command line asks for the create page: `--new`, or the URI the desktop portal passes
/// when Sticky Notes opens `organiser-<slug>://new`.
pub fn wants_new(meta: &AppMeta) -> bool {
    let prefix = format!("{}:", scheme(meta));
    std::env::args()
        .skip(1)
        .any(|a| a == "--new" || a.starts_with(&prefix))
}

/// Whether to draw dark. A forced `mode` wins; otherwise iad's desktop settings decide: a
/// `color_scheme` fixed in `desktop.ron` is used, and `system` (the default) follows what the
/// settings portal reports (`desktop_dark`).
pub fn is_dark(mode: crate::primitives::scheme::Mode, desktop_dark: bool) -> bool {
    use crate::primitives::scheme::Mode;
    match (mode, &iad::config::current().color_scheme) {
        (Mode::Follow, iad::config::Pick::Fixed(s)) => matches!(s, iad::Scheme::Dark),
        _ => mode.is_dark(desktop_dark),
    }
}

/// The bundled base and selected theme overlay, with desktop settings and the given scheme.
pub fn theme(dark: bool) -> iad::Theme {
    let mut desktop = (*iad::config::current()).clone();
    desktop.color_scheme = iad::config::Pick::Fixed(if dark {
        iad::Scheme::Dark
    } else {
        iad::Scheme::Light
    });
    iad::Theme::from_config(&desktop)
}

/// Open a link from a note in the desktop's default handler. Only web and mail links are opened
/// (see [`crate::primitives::link::is_openable`]); the error says why nothing happened.
pub fn open_link(url: &str) -> Result<(), String> {
    if !crate::primitives::link::is_openable(url) {
        return Err(format!("Not opening {url}: only web and mail links open"));
    }
    std::process::Command::new("xdg-open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Cannot open {url}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve_theme(overlay: &str) -> iad::theme_config::ThemeConfig {
        iad::theme_config::ThemeConfig::builder("organiser-ui-test")
            .files(false)
            .base_theme(DEFAULT_BASE_THEME)
            .toolkit_text(overlay)
            .build()
    }

    #[test]
    fn bundled_theme_is_valid_and_styles_both_schemes() {
        let config = resolve_theme("");
        assert!(config.warnings().is_empty(), "{:?}", config.warnings());
        assert!(config.unknown().is_empty(), "{:?}", config.unknown());
        for theme in [iad::Theme::light(), iad::Theme::dark()] {
            let theme = theme.with_theme_config(&config);
            assert_eq!(theme.metrics.radius_window, 15.0);
            assert_eq!(theme.metrics.space_m, 12.0);
        }
    }

    #[test]
    fn sparse_overlay_keeps_base_geometry_and_rejects_invalid_values() {
        let config = resolve_theme(
            r##"{ "metrics": { "radius-window": 21, "space-m": "invalid" }, "colors": { "window-bg": "#123456" } }"##,
        );
        let theme = iad::Theme::light().with_theme_config(&config);
        assert_eq!(theme.metrics.radius_window, 21.0);
        assert_eq!(theme.metrics.space_m, 12.0);
        assert_eq!(
            theme.palette.window_bg_color.into_rgba8(),
            [0x12, 0x34, 0x56, 0xff]
        );
        assert!(!config.warnings().is_empty());
    }

    #[test]
    fn forced_mode_beats_desktop_and_system_follows_it() {
        use crate::primitives::scheme::Mode;
        // No desktop.ron is installed in tests, so the scheme is `system`.
        assert!(is_dark(Mode::Follow, true));
        assert!(!is_dark(Mode::Follow, false));
        assert!(is_dark(Mode::Dark, false));
        assert!(!is_dark(Mode::Light, true));
    }
}
