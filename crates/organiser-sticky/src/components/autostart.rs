//! Starting Sticky on login, as `sticky-gui --background`, which shows the stickies for the desktop
//! and ends at once if there are none.
//!
//! Outside a sandbox that is an XDG autostart entry. Inside a Flatpak the app may not write the
//! host's `~/.config/autostart`, so it asks the Background portal (`RequestBackground` with
//! `autostart`); the desktop may ask the person to confirm. The portal's answer arrives later and
//! cannot be read back, so what was asked is remembered in a small marker file.

use std::path::{Path, PathBuf};

use crate::STICKY_ID;
use crate::components::store::in_flatpak;

/// `~/.config/autostart/<id>.desktop` (the host's, also from inside a Flatpak).
pub fn entry_path() -> PathBuf {
    let base = if in_flatpak() {
        std::env::var_os("HOME").map(|h| Path::new(&h).join(".config"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".config")))
    }
    .unwrap_or_else(|| PathBuf::from(".config"));
    base.join("autostart").join(format!("{STICKY_ID}.desktop"))
}

/// The entry's text, running `exec`.
pub fn render(exec: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Sticky Notes\nComment=Show your stickies on login\n\
         Exec={exec}\nIcon={STICKY_ID}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
    )
}

fn exec() -> String {
    if in_flatpak() {
        format!("flatpak run {STICKY_ID} --background")
    } else {
        let exe = std::env::current_exe()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "sticky-gui".into());
        format!("{exe} --background")
    }
}

/// What was last asked of the Background portal (inside a sandbox, where nothing can be read
/// back).
fn marker_path() -> PathBuf {
    crate::components::store::dir().join("sticky-autostart")
}

pub fn is_enabled() -> bool {
    if in_flatpak() {
        marker_path().exists()
    } else {
        entry_path().exists()
    }
}

/// The options of `RequestBackground`, as `gdbus` writes a variant dictionary.
pub fn portal_options(on: bool) -> String {
    if on {
        "{'autostart': <true>, 'commandline': <['sticky-gui-launch', '--background']>, \
         'reason': <'Show your stickies when you log in'>}"
            .into()
    } else {
        "{'autostart': <false>}".into()
    }
}

pub fn set_enabled(on: bool) -> Result<(), String> {
    if !in_flatpak() {
        return set_enabled_at(&entry_path(), &exec(), on);
    }
    crate::components::discover::portal(
        "org.freedesktop.portal.Background.RequestBackground",
        &["", &portal_options(on)],
    )?;
    let marker = marker_path();
    if on {
        if let Some(dir) = marker.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        }
        std::fs::write(&marker, "requested\n")
            .map_err(|e| format!("cannot write {}: {e}", marker.display()))
    } else {
        match std::fs::remove_file(&marker) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("cannot remove {}: {e}", marker.display())),
        }
    }
}

pub fn set_enabled_at(path: &Path, exec: &str, on: bool) -> Result<(), String> {
    if on {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        }
        std::fs::write(path, render(exec))
            .map_err(|e| format!("cannot write {}: {e}", path.display()))
    } else {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("cannot remove {}: {e}", path.display())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_portal_options_are_a_variant_dictionary() {
        let on = portal_options(true);
        assert!(on.starts_with("{'autostart': <true>"));
        assert!(on.contains("'commandline': <['sticky-gui-launch', '--background']>"));
        assert_eq!(portal_options(false), "{'autostart': <false>}");
    }

    #[test]
    fn enabling_writes_the_entry_and_disabling_removes_it() {
        let dir =
            std::env::temp_dir().join(format!("organiser-sticky-auto-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = dir.join("autostart").join("x.desktop");
        set_enabled_at(&p, "sticky-gui --background", true).unwrap();
        let t = std::fs::read_to_string(&p).unwrap();
        assert!(t.contains("Exec=sticky-gui --background\n"));
        set_enabled_at(&p, "", false).unwrap();
        assert!(!p.exists());
        set_enabled_at(&p, "", false).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
