//! Quick add: `Buy milk #home #errands` is a note titled "Buy milk" with two tags.

/// What a quick-add line means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quick {
    pub title: String,
    pub tags: Vec<String>,
}

pub fn parse(line: &str) -> Quick {
    let mut words = Vec::new();
    let mut tags: Vec<String> = Vec::new();
    for w in line.split_whitespace() {
        match w.strip_prefix('#').filter(|t| !t.is_empty()) {
            Some(t) if !tags.iter().any(|x| x.eq_ignore_ascii_case(t)) => tags.push(t.into()),
            // A repeated tag is dropped, not left behind in the title.
            Some(_) => {}
            None => words.push(w),
        }
    }
    Quick {
        title: words.join(" "),
        tags,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashtags_become_tags_and_the_rest_the_title() {
        let q = parse("Buy  milk #home #errands");
        assert_eq!(q.title, "Buy milk");
        assert_eq!(q.tags, ["home", "errands"]);
    }

    #[test]
    fn repeated_tags_collapse_and_a_lone_hash_stays_text() {
        let q = parse("# idea #Work #work");
        assert_eq!(q.title, "# idea");
        assert_eq!(q.tags, ["Work"]);
    }

    #[test]
    fn only_tags_leaves_an_empty_title() {
        assert_eq!(parse("#a").title, "");
    }
}
