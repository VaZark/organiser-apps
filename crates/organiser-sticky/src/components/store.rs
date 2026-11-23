//! The registry file: `$XDG_CONFIG_HOME/organiser/sticky-links.toml`, shared by every app.

use std::path::{Path, PathBuf};

use organiser_core::paths;
use organiser_core::sticky::StickySpec;

use crate::primitives::local::Local;
use crate::primitives::registry::{Link, Links};

/// Inside a Flatpak sandbox the app's own `XDG_CONFIG_HOME` is private to it, so the registry
/// (which every app and Sticky must see) lives in the host's `~/.config/organiser`, which the
/// manifests share.
pub fn in_flatpak() -> bool {
    Path::new("/.flatpak-info").exists()
}

pub fn dir() -> PathBuf {
    match std::env::var_os("HOME") {
        Some(h) if in_flatpak() => Path::new(&h).join(".config").join("organiser"),
        _ => paths::shared_config_dir(),
    }
}

pub fn path() -> PathBuf {
    dir().join("sticky-links.toml")
}

/// Sticky Notes' own per-item state (shown on the desktop, colour); see [`crate::Local`].
pub fn local_path() -> PathBuf {
    dir().join("sticky-local.toml")
}

/// Read it; a missing file is no entries (everything at its defaults).
pub fn load_local() -> Result<Local, String> {
    let p = local_path();
    match std::fs::read_to_string(&p) {
        Ok(t) => Local::parse(&t).map_err(|e| format!("{} is not valid: {e}", p.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Local::default()),
        Err(e) => Err(format!("cannot read {}: {e}", p.display())),
    }
}

/// Write it whole or not at all.
pub fn save_local(local: &Local) -> Result<(), String> {
    let p = local_path();
    write_atomic(&p, &local.render()?)
}

/// Read the registry: a missing file is an empty registry.
pub fn load() -> Result<Links, String> {
    load_from(&path())
}

pub fn load_from(path: &Path) -> Result<Links, String> {
    match std::fs::read_to_string(path) {
        Ok(t) => Links::parse(&t).map_err(|e| format!("{} is not valid: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Links::default()),
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

/// Write the registry whole or not at all.
pub fn save_to(path: &Path, links: &Links) -> Result<(), String> {
    write_atomic(path, &links.render()?)
}

fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("cannot replace {}: {e}", path.display()))
}

/// An app's offer, with its database at the default place.
pub fn link_for(spec: &StickySpec) -> Link {
    Link::from_spec(
        spec,
        paths::db_path(&spec.meta).to_string_lossy().into_owned(),
    )
}

/// Publish an app to Sticky Notes: add it to the registry (switched off) or refresh what it says
/// about itself. Apps call this at start; nothing is written unless something changed.
pub fn publish(spec: &StickySpec) -> Result<(), String> {
    let p = path();
    let mut links = load_from(&p)?;
    if links.publish(link_for(spec)) {
        save_to(&p, &links)?;
    }
    Ok(())
}

/// Whether the app is published and switched on.
pub fn is_linked(spec: &StickySpec) -> bool {
    load().is_ok_and(|l| l.is_linked(spec.meta.id))
}

/// Switch an app on or off (publishing it first).
pub fn set_linked(spec: &StickySpec, on: bool) -> Result<(), String> {
    set_enabled(&link_for(spec), on)
}

/// Switch a published app on or off, adding `link` first if it is new.
pub fn set_enabled(link: &Link, on: bool) -> Result<(), String> {
    let p = path();
    let mut links = load_from(&p)?;
    links.publish(link.clone());
    links.set_enabled(&link.app_id, on);
    save_to(&p, &links)
}

/// Switch a published app on or off by its id (from Sticky Notes' own preferences).
pub fn set_enabled_by_id(app_id: &str, on: bool) -> Result<(), String> {
    let p = path();
    let mut links = load_from(&p)?;
    if links.set_enabled(app_id, on) {
        save_to(&p, &links)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_is_an_empty_registry_and_saving_creates_it() {
        let dir =
            std::env::temp_dir().join(format!("organiser-sticky-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = dir.join("sub").join("links.toml");
        assert_eq!(load_from(&p).unwrap(), Links::default());
        let mut l = Links::default();
        l.publish(Link {
            app_id: "a".into(),
            slug: "a".into(),
            db_path: "/x".into(),
            ..Link::default()
        });
        save_to(&p, &l).unwrap();
        assert_eq!(load_from(&p).unwrap(), l);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
