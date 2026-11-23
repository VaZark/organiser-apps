//! Finding the Sticky app and starting it.
//!
//! Outside a sandbox that is a look at `PATH` (or an exported Flatpak). Inside a Flatpak sandbox
//! the host is not visible, and a sandboxed app has no business running host programs, so the
//! desktop portals do it: Sticky registers a URI scheme (`organiser-sticky:`) in its desktop file,
//! `OpenURI.SchemeSupported` says whether any installed app handles it, and opening a URI of that
//! scheme starts it. No permission beyond the portals everyone has is needed.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::components::store::in_flatpak;
use crate::{STICKY_BIN, STICKY_ID};

/// The URI scheme Sticky Notes handles, declared in its desktop file.
pub const SCHEME: &str = "organiser-sticky";
/// The URI that starts Sticky as `--background` does.
pub const START_URI: &str = "organiser-sticky://start";

/// How to start Sticky.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launcher {
    /// A binary on `PATH` (or an absolute path).
    Binary(PathBuf),
    /// An installed Flatpak, from outside a sandbox: `flatpak run <id>`.
    Flatpak,
    /// From inside a sandbox: open [`START_URI`] through the desktop portal.
    Portal,
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

/// `gdbus call` on the session bus to the desktop portal; its text output, if it worked.
pub(crate) fn portal(method: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.freedesktop.portal.Desktop",
            "--object-path",
            "/org/freedesktop/portal/desktop",
            "--method",
            method,
        ])
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot reach the desktop portal (gdbus): {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// `(true,)` or `(false,)`, as `gdbus` prints a boolean reply.
pub fn parse_bool_reply(text: &str) -> Option<bool> {
    match text.trim() {
        "(true,)" => Some(true),
        "(false,)" => Some(false),
        _ => None,
    }
}

/// Whether the desktop portal knows an installed app that handles Sticky's URI scheme.
pub fn scheme_supported() -> bool {
    portal(
        "org.freedesktop.portal.OpenURI.SchemeSupported",
        &[SCHEME, "{}"],
    )
    .ok()
    .and_then(|t| parse_bool_reply(&t))
    .unwrap_or(false)
}

/// Where Sticky is installed, if it is. In a sandbox, the portal says; outside, `sticky-gui` on
/// `PATH` (also reached by the wrapper a native install puts there), else an exported Flatpak.
pub fn find() -> Option<Launcher> {
    if in_flatpak() {
        return scheme_supported().then_some(Launcher::Portal);
    }
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

/// Start Sticky as `--background` does: it shows the stickies for the desktop and ends if there are
/// none. Safe to call when it already runs: the running one watches the files and picks up changes.
pub fn launch() -> Result<(), String> {
    let l = find().ok_or("Sticky Notes is not installed")?;
    let mut cmd = match l {
        // `xdg-open` in a sandbox is the OpenURI portal.
        Launcher::Portal => {
            let mut c = Command::new("xdg-open");
            c.arg(START_URI);
            c
        }
        Launcher::Binary(p) => {
            let mut c = Command::new(p);
            c.arg("--background");
            c
        }
        Launcher::Flatpak => {
            let mut c = Command::new("flatpak");
            c.args(["run", STICKY_ID, "--background"]);
            c
        }
    };
    cmd.stdin(Stdio::null())
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

/// The binary names an app's GUI has had, newest first. Our own app is looked for before anything
/// else is tried.
fn gui_names(slug: &str) -> [String; 2] {
    [format!("{slug}-kirigami"), format!("{slug}-gui")]
}

/// The first GUI binary of `slug` found in `dirs`, in the order of [`gui_names`].
fn find_gui(dirs: &[PathBuf], slug: &str) -> Option<PathBuf> {
    gui_names(slug)
        .iter()
        .find_map(|n| dirs.iter().map(|d| d.join(n)).find(|p| is_executable(p)))
}

/// Where our install script puts binaries, in case it is not on `PATH`.
fn install_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/bin"))
}

/// Open an app on its create page: `--new 1` (a value, as the apps read their flags). In a sandbox
/// the app is reached through the desktop portal, by the URI scheme it declares
/// (`organiser-<slug>://new`); outside, our own GUI is searched for first (`<slug>-kirigami` on
/// `PATH`, then in `~/.local/bin`), then the older `<slug>-gui`, then its Flatpak.
pub fn open_new(slug: &str, app_id: &str) -> Result<(), String> {
    let mut cmd = if in_flatpak() {
        let mut c = Command::new("xdg-open");
        c.arg(format!("organiser-{slug}://new"));
        c
    } else if let Some(bin) = {
        let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect())
            .unwrap_or_default();
        dirs.extend(install_dir());
        find_gui(&dirs, slug)
    } {
        let mut c = Command::new(bin);
        c.args(["--new", "1"]);
        c
    } else {
        let mut c = Command::new("flatpak");
        c.args(["run", app_id, "--new", "1"]);
        c
    };
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("cannot start {app_id}: {e}"))
}

/// Whether `args` (a command line's arguments) ask for Sticky to start in the background: the
/// `--background` flag, or the URI the portal passes when it opens [`START_URI`].
pub fn wants_background<S: AsRef<str>>(args: &[S]) -> bool {
    args.iter().any(|a| {
        let a = a.as_ref();
        a == "--background" || a.starts_with(&format!("{SCHEME}:"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn our_own_gui_is_found_before_the_old_one() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("sticky-gui-find-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let make = |name: &str| {
            let p = dir.join(name);
            std::fs::write(&p, "#!/bin/sh\n").unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        };
        let dirs = [dir.clone()];
        assert_eq!(find_gui(&dirs, "todo"), None);
        make("todo-gui");
        assert_eq!(find_gui(&dirs, "todo"), Some(dir.join("todo-gui")));
        make("todo-kirigami");
        assert_eq!(find_gui(&dirs, "todo"), Some(dir.join("todo-kirigami")));
        assert_eq!(find_gui(&dirs, "notes"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_gdbus_boolean_reply_is_read() {
        assert_eq!(parse_bool_reply("(true,)\n"), Some(true));
        assert_eq!(parse_bool_reply("(false,)"), Some(false));
        assert_eq!(parse_bool_reply("(1,)"), None);
        assert_eq!(parse_bool_reply(""), None);
    }

    #[test]
    fn sticky_starts_in_the_background_from_its_flag_or_its_uri() {
        assert!(wants_background(&["--background"]));
        assert!(wants_background(&["organiser-sticky://start"]));
        assert!(wants_background(&["organiser-sticky:start"]));
        assert!(!wants_background(&["--print-desktop-entry"]));
        assert!(!wants_background::<&str>(&[]));
        assert!(!wants_background(&["https://example.org"]));
    }
}
