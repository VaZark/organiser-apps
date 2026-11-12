//! Finding the Sticky app and starting it.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::components::store::in_flatpak;
use crate::{STICKY_BIN, STICKY_ID};

/// How to start Sticky.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launcher {
    /// A binary on `PATH` (or an absolute path).
    Binary(PathBuf),
    /// An installed Flatpak: `flatpak run <id>`.
    Flatpak,
}

fn on_path(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|d| d.join(name))
        .find(|p| is_executable(p))
}

fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

fn flatpak_exports() -> Vec<PathBuf> {
    let mut v = vec![PathBuf::from("/var/lib/flatpak/exports/bin")];
    if let Some(h) = std::env::var_os("HOME") {
        v.push(Path::new(&h).join(".local/share/flatpak/exports/bin"));
    }
    v
}

/// Where Sticky is installed, if it is: `sticky-gui` on `PATH` (also reached by the wrapper a
/// native install puts there), else an exported Flatpak.
pub fn find() -> Option<Launcher> {
    if let Some(p) = on_path(STICKY_BIN) {
        return Some(Launcher::Binary(p));
    }
    flatpak_exports()
        .iter()
        .any(|d| is_executable(&d.join(STICKY_ID)))
        .then_some(Launcher::Flatpak)
}

pub fn available() -> bool {
    find().is_some()
}

/// `flatpak run <Sticky>`; from inside a sandbox it is run on the host (the manifests grant
/// `--talk-name=org.freedesktop.Flatpak` for this).
fn host_flatpak_run() -> Command {
    let (program, lead): (&str, &[&str]) = if in_flatpak() {
        ("flatpak-spawn", &["--host", "flatpak", "run"])
    } else {
        ("flatpak", &["run"])
    };
    let mut c = Command::new(program);
    c.args(lead).arg(STICKY_ID);
    c
}

/// Start Sticky (`--background`: it shows the flagged items and ends if there are none). Safe to
/// call when it already runs: it only lets the running one know by existing; the daemon polls.
pub fn launch() -> Result<(), String> {
    let l = find().ok_or("Sticky Notes is not installed")?;
    let mut cmd = match l {
        Launcher::Binary(p) => Command::new(p),
        Launcher::Flatpak => host_flatpak_run(),
    };
    cmd.arg("--background")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("cannot start Sticky Notes: {e}"))
}
