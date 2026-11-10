//! The layer rule, enforced: `primitives` has no iced and imports no other layer; `components`
//! imports only `primitives`; `widgets` only `components` and `primitives`.

use std::fs;
use std::path::Path;

const LAYERS: &[&str] = &["primitives", "components", "widgets", "shell"];
const ALLOWED: &[(&str, &[&str])] = &[
    ("primitives", &[]),
    ("components", &["primitives"]),
    ("widgets", &["components", "primitives"]),
];

fn code(src: &str) -> String {
    src.lines()
        .take_while(|l| !l.trim_start().starts_with("#[cfg(test)]"))
        .filter(|l| !l.trim_start().starts_with("//"))
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

fn imported(code: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = code;
    while let Some(i) = rest.find("crate::") {
        rest = &rest[i + 7..];
        if let Some(g) = rest.strip_prefix('{') {
            let end = g.find('}').unwrap_or(g.len());
            found.extend(
                g[..end]
                    .split(',')
                    .map(|p| p.trim().split("::").next().unwrap_or("").to_string()),
            );
        } else {
            found.push(
                rest.split(|c: char| !c.is_alphanumeric() && c != '_')
                    .next()
                    .unwrap_or("")
                    .to_string(),
            );
        }
    }
    found.retain(|n| LAYERS.contains(&n.as_str()));
    found
}

#[test]
fn layers_import_only_what_is_below() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut bad = Vec::new();
    for (layer, may) in ALLOWED {
        for e in fs::read_dir(src.join(layer)).unwrap().flatten() {
            let c = code(&fs::read_to_string(e.path()).unwrap());
            for n in imported(&c) {
                if n != *layer && !may.contains(&n.as_str()) {
                    bad.push(format!("{}: {layer} imports {n}", e.path().display()));
                }
            }
            if *layer == "primitives" && (c.contains("iced") || c.contains("iad")) {
                bad.push(format!(
                    "{}: primitives mention iced/iad",
                    e.path().display()
                ));
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
