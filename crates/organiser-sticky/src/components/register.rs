//! Registering an app with Sticky Notes without compiling anything in: a registration file in the
//! drop-in folder ([`store::dropin_dir`]). Any program can write that file itself (the format is
//! the `[[link]]` table of `sticky-links.toml`, documented in the crate's README), or call
//! `sticky-gui register`, which does it with these functions.

use std::path::{Path, PathBuf};

use crate::components::store;
use crate::primitives::registry::{KindSpec, Link, Links};

/// An app id usable as a file name: letters, digits, `.`, `_` and `-`, and not a path.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('.')
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// A name is not empty and is a plain SQL-free identifier: kinds and field names go into JSON paths.
pub fn valid_name(n: &str) -> bool {
    !n.is_empty() && n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Check a registration before it is written.
pub fn validate(link: &Link) -> Result<(), String> {
    if !valid_id(&link.app_id) {
        return Err(format!("{:?} is not a usable app id", link.app_id));
    }
    if link.db_path.is_empty() {
        return Err("a database path is needed".into());
    }
    if link.kinds.is_empty() {
        return Err("at least one kind is needed (kind=field)".into());
    }
    for k in &link.kinds {
        if !valid_name(&k.kind) || !valid_name(&k.body) {
            return Err(format!(
                "kind {:?} / field {:?}: use letters, digits and _ only",
                k.kind, k.body
            ));
        }
    }
    Ok(())
}

/// Parse `kind=field`.
pub fn parse_kind(s: &str) -> Result<KindSpec, String> {
    let (kind, body) = s
        .split_once('=')
        .ok_or_else(|| format!("{s:?}: expected kind=field"))?;
    Ok(KindSpec {
        kind: kind.into(),
        body: body.into(),
    })
}

pub fn dropin_path(dir: &Path, app_id: &str) -> PathBuf {
    dir.join(format!("{app_id}.toml"))
}

/// Write the registration of `link` into `dir`. It is offered switched off; linking is separate
/// (see [`store::set_enabled_by_id`]).
pub fn register_in(dir: &Path, link: &Link) -> Result<PathBuf, String> {
    validate(link)?;
    let one = Links {
        links: vec![Link {
            enabled: false,
            ..link.clone()
        }],
    };
    let path = dropin_path(dir, &link.app_id);
    store::save_to(&path, &one)?;
    Ok(path)
}

/// Remove a registration; whether there was one.
pub fn unregister_in(dir: &Path, app_id: &str) -> Result<bool, String> {
    if !valid_id(app_id) {
        return Err(format!("{app_id:?} is not a usable app id"));
    }
    match std::fs::remove_file(dropin_path(dir, app_id)) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("cannot remove the registration: {e}")),
    }
}

pub fn register(link: &Link) -> Result<PathBuf, String> {
    register_in(&store::dropin_dir(), link)
}

/// Remove a registration, the app's entry in the links file, and everything Sticky Notes kept about
/// the app's items (their desktop visibility and colours), so it is gone from Sticky entirely.
pub fn unregister(app_id: &str) -> Result<(), String> {
    unregister_in(&store::dropin_dir(), app_id)?;
    let p = store::path();
    let mut links = store::load_from(&p)?;
    if links.remove(app_id) {
        store::save_to(&p, &links)?;
    }
    let mut local = store::load_local()?;
    if local.clear_app(app_id) {
        store::save_local(&local)?;
    }
    Ok(())
}
