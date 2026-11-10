//! Parsing the little languages of the command line: `key=value`, filters and sorts.

use organiser_engine::Op;

/// `key=value` → (`key`, `value`).
pub fn key_value(s: &str) -> Result<(&str, &str), String> {
    s.split_once('=')
        .filter(|(k, _)| !k.trim().is_empty())
        .map(|(k, v)| (k.trim(), v))
        .ok_or_else(|| format!("expected key=value, got {s:?}"))
}

/// `due<2026-12-01`, `status!=done`, `title~milk`, `stars>=3` → (field, op, raw value).
pub fn filter(s: &str) -> Result<(&str, Op, &str), String> {
    for (sym, op) in [
        ("!=", Op::Ne),
        (">=", Op::Ge),
        ("<=", Op::Le),
        ("=", Op::Eq),
        (">", Op::Gt),
        ("<", Op::Lt),
        ("~", Op::Contains),
    ] {
        if let Some((k, v)) = s.split_once(sym)
            && !k.trim().is_empty()
            && k.trim().chars().all(|c| c.is_alphanumeric() || c == '_')
        {
            return Ok((k.trim(), op, v));
        }
    }
    Err(format!(
        "expected a filter like field=value, field<value, field~text; got {s:?}"
    ))
}

/// `due` or `due:desc` → (field, descending).
pub fn sort(s: &str) -> (&str, bool) {
    match s.split_once(':') {
        Some((f, d)) => (f, d.eq_ignore_ascii_case("desc")),
        None => (s, false),
    }
}

/// `root` → no parent; a number → that item.
pub fn parent(s: &str) -> Result<Option<i64>, String> {
    if s.eq_ignore_ascii_case("root") || s.is_empty() {
        return Ok(None);
    }
    s.parse()
        .map(Some)
        .map_err(|_| format!("expected an id or `root`, got {s:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_specs() {
        assert_eq!(key_value("a=b=c").unwrap(), ("a", "b=c"));
        assert!(key_value("=x").is_err());
        assert_eq!(filter("status!=done").unwrap(), ("status", Op::Ne, "done"));
        assert_eq!(filter("due<=2026-01-01").unwrap().1, Op::Le);
        assert_eq!(filter("title~a=b").unwrap(), ("title", Op::Contains, "a=b"));
        assert!(filter("nothing").is_err());
        assert_eq!(sort("due:DESC"), ("due", true));
        assert_eq!(sort("due"), ("due", false));
        assert_eq!(parent("root").unwrap(), None);
        assert_eq!(parent("7").unwrap(), Some(7));
        assert!(parent("x").is_err());
    }
}
