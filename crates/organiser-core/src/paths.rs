//! Per-app XDG locations. Each app has its own directory and its own database file.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::AppMeta;

/// `$XDG_DATA_HOME/<slug>`, else `~/.local/share/<slug>`.
pub fn data_dir(meta: &AppMeta) -> PathBuf {
    resolve(env("XDG_DATA_HOME"), env("HOME"), ".local/share", meta.slug)
}

/// `$XDG_CONFIG_HOME/<slug>`, else `~/.config/<slug>`.
pub fn config_dir(meta: &AppMeta) -> PathBuf {
    resolve(env("XDG_CONFIG_HOME"), env("HOME"), ".config", meta.slug)
}

/// `$XDG_CONFIG_HOME/organiser`: what every organiser app shares (settings, the Sticky links).
pub fn shared_config_dir() -> PathBuf {
    resolve(env("XDG_CONFIG_HOME"), env("HOME"), ".config", "organiser")
}

/// The app's database file: `<data_dir>/<slug>.db`.
pub fn db_path(meta: &AppMeta) -> PathBuf {
    data_dir(meta).join(format!("{}.db", meta.slug))
}

fn env(name: &str) -> Option<OsString> {
    std::env::var_os(name)
}

/// The XDG rule: a base directory variable is honoured only when it is an absolute path.
fn resolve(xdg: Option<OsString>, home: Option<OsString>, fallback: &str, slug: &str) -> PathBuf {
    let base = xdg
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home.map(|h| Path::new(&h).join(fallback)))
        .unwrap_or_else(|| PathBuf::from(".").join(fallback));
    base.join(slug)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(s: &str) -> Option<OsString> {
        Some(s.into())
    }

    #[test]
    fn absolute_xdg_wins() {
        let p = resolve(os("/x/data"), os("/home/u"), ".local/share", "notes");
        assert_eq!(p, PathBuf::from("/x/data/notes"));
    }

    #[test]
    fn relative_or_missing_xdg_falls_back_to_home() {
        let want = PathBuf::from("/home/u/.local/share/notes");
        assert_eq!(
            resolve(os("rel"), os("/home/u"), ".local/share", "notes"),
            want
        );
        assert_eq!(resolve(None, os("/home/u"), ".local/share", "notes"), want);
    }
}
