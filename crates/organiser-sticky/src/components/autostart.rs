//! Starting Sticky on login: an XDG autostart entry that runs `sticky-gui --background`, which
//! shows the flagged items and ends at once if there are none.

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

pub fn is_enabled() -> bool {
    entry_path().exists()
}

pub fn set_enabled(on: bool) -> Result<(), String> {
    set_enabled_at(&entry_path(), &exec(), on)
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
