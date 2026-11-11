//! Which links a note may open.

/// Whether pressing a link in a note may hand it to the desktop: web pages and mail addresses
/// only. Anything else (a file path, `javascript:`, a custom scheme) would let a note run
/// something on the reader's machine, so it is not opened.
pub fn is_openable(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    ["http://", "https://", "mailto:"]
        .iter()
        .any(|p| u.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_and_mail_links_open_and_nothing_else_does() {
        for ok in ["https://duckduckgo.com", "HTTP://x.org", " mailto:a@b.c"] {
            assert!(is_openable(ok), "{ok}");
        }
        for no in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "/home/me",
            "ftp://x",
            "",
            "img.png",
        ] {
            assert!(!is_openable(no), "{no}");
        }
    }
}
