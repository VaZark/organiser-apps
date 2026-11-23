//! The layer rule, enforced: each layer of `src/` imports only the layers below it. Comments and
//! `#[cfg(test)]` code are ignored.

use std::fs;
use std::path::{Path, PathBuf};

/// (layer, the layers it may import besides itself)
const ALLOWED: &[(&str, &[&str])] = &[("primitives", &[]), ("components", &["primitives"])];
const LAYERS: &[&str] = &["primitives", "components", "db"];
/// Not allowed in the pure layer.
const NOT_IN_PRIMITIVES: &[&str] = &["rusqlite"];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

fn code(src: &str) -> String {
    let mut out = String::new();
    for line in src.lines() {
        let t = line.trim_start();
        if t.starts_with("#[cfg(test)]") {
            break;
        }
        if t.starts_with("//") {
            continue;
        }
        out.push_str(line.split("//").next().unwrap_or(""));
        out.push('\n');
    }
    out
}

/// Every layer named after `crate::`, including inside `crate::{a, b::c}`.
fn imported(code: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = code;
    while let Some(i) = rest.find("crate::") {
        rest = &rest[i + 7..];
        if let Some(group) = rest.strip_prefix('{') {
            let end = group.find('}').unwrap_or(group.len());
            for part in group[..end].split(',') {
                found.push(part.trim().split("::").next().unwrap_or("").to_string());
            }
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
        let mut files = Vec::new();
        rust_files(&src.join(layer), &mut files);
        assert!(!files.is_empty(), "{layer} has no files");
        for f in files {
            let c = code(&fs::read_to_string(&f).unwrap());
            for name in imported(&c) {
                if name != *layer && !may.contains(&name.as_str()) {
                    bad.push(format!("{}: {layer} imports {name}", f.display()));
                }
            }
            if *layer == "primitives" {
                for n in NOT_IN_PRIMITIVES {
                    if c.contains(n) {
                        bad.push(format!("{}: primitives mention {n}", f.display()));
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "layer violations:\n{}", bad.join("\n"));
}
