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

/// The person's documents folder: `$XDG_DOCUMENTS_DIR`, else the one `~/.config/user-dirs.dirs`
/// names, else `~/Documents`.
pub fn documents_dir() -> PathBuf {
    let home = env("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    if let Some(d) = env("XDG_DOCUMENTS_DIR").map(PathBuf::from).filter(|p| p.is_absolute()) {
        return d;
    }
    let config = resolve(env("XDG_CONFIG_HOME"), env("HOME"), ".config", "");
    let file = config.parent().map(|p| p.join("user-dirs.dirs"));
    file.and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|t| user_dir(&t, "XDG_DOCUMENTS_DIR", &home))
        .unwrap_or_else(|| home.join("Documents"))
}

/// Where an app keeps its files by default: a folder named for it in the documents folder.
pub fn documents_folder(meta: &AppMeta) -> PathBuf {
    documents_dir().join(meta.name)
}

/// `KEY="$HOME/Documents"` of a `user-dirs.dirs` file, as a path.
fn user_dir(text: &str, key: &str, home: &Path) -> Option<PathBuf> {
    text.lines().find_map(|l| {
        let v = l.trim().strip_prefix(key)?.trim_start().strip_prefix('=')?;
        let v = v.trim().trim_matches('"');
        let path = match v.strip_prefix("$HOME") {
            Some(rest) => home.join(rest.trim_start_matches('/')),
            None => PathBuf::from(v),
        };
        (path.is_absolute() && path != home).then_some(path)
    })
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
    fn the_documents_folder_is_read_from_user_dirs() {
        let home = Path::new("/home/u");
        let text = "# comment\nXDG_DESKTOP_DIR=\"$HOME/Desktop\"\nXDG_DOCUMENTS_DIR=\"$HOME/Dokumente\"\n";
        assert_eq!(user_dir(text, "XDG_DOCUMENTS_DIR", home), Some(PathBuf::from("/home/u/Dokumente")));
        assert_eq!(user_dir("XDG_DOCUMENTS_DIR=\"/data/docs\"", "XDG_DOCUMENTS_DIR", home), Some(PathBuf::from("/data/docs")));
        // "$HOME" alone means the folder is not set apart.
        assert_eq!(user_dir("XDG_DOCUMENTS_DIR=\"$HOME/\"", "XDG_DOCUMENTS_DIR", home), None);
        assert_eq!(user_dir(text, "XDG_MISSING_DIR", home), None);
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
