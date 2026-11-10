//! `primitives` must stay pure: no clap, no engine database, nothing from `components`.

use std::fs;

#[test]
fn primitives_are_pure() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/primitives");
    for e in fs::read_dir(dir).unwrap().flatten() {
        let src = fs::read_to_string(e.path()).unwrap();
        let code: String = src
            .lines()
            .take_while(|l| !l.trim_start().starts_with("#[cfg(test)]"))
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for bad in [
            "clap",
            "crate::components",
            "organiser_engine::Db",
            "organiser_engine::{Db",
        ] {
            assert!(!code.contains(bad), "{} mentions {bad}", e.path().display());
        }
    }
}
