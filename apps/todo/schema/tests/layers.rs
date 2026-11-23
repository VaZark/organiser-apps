//! `primitives` must stay pure: no engine database, and nothing from `components`.

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
            "crate::components",
            "crate::lifecycle",
            "crate::views",
            "organiser_engine::Db",
            "rusqlite",
        ] {
            assert!(!code.contains(bad), "{} mentions {bad}", e.path().display());
        }
    }
}
