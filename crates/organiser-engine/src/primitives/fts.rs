//! Turning what a person typed into a safe FTS5 expression.

/// Every word becomes a quoted prefix term, all of them required: `pasta sau` → `"pasta"* "sau"*`.
/// Words without a letter or digit are dropped. `None` when nothing searchable is left.
pub fn match_expr(text: &str) -> Option<String> {
    let terms: Vec<String> = text
        .split_whitespace()
        .map(|w| w.replace('"', ""))
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .map(|w| format!("\"{w}\"*"))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_and_drops_noise() {
        assert_eq!(match_expr("pasta sau").unwrap(), "\"pasta\"* \"sau\"*");
        assert_eq!(match_expr("a\"b - ").unwrap(), "\"ab\"*");
        assert_eq!(match_expr("  - ** "), None);
    }
}
