//! How many tags a crowded row shows.

/// The tags to draw, and how many more were left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collapsed<'a> {
    pub shown: Vec<&'a str>,
    pub more: usize,
}

/// Up to `limit` tags are shown as they are. With more than that, the first tag stands for them
/// and the rest are counted: `home, work, travel` with a limit of 2 → `home` and `+2`.
pub fn collapse(tags: &[String], limit: usize) -> Collapsed<'_> {
    if tags.len() <= limit {
        Collapsed {
            shown: tags.iter().map(String::as_str).collect(),
            more: 0,
        }
    } else {
        Collapsed {
            shown: tags.iter().take(1).map(String::as_str).collect(),
            more: tags.len() - 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn shows_up_to_the_limit_then_one_and_a_count() {
        assert_eq!(
            collapse(&[], 2),
            Collapsed {
                shown: vec![],
                more: 0
            }
        );
        assert_eq!(collapse(&v(&["a"]), 2).shown, ["a"]);
        assert_eq!(
            collapse(&v(&["a", "b"]), 2),
            Collapsed {
                shown: vec!["a", "b"],
                more: 0
            }
        );
        assert_eq!(
            collapse(&v(&["a", "b", "c"]), 2),
            Collapsed {
                shown: vec!["a"],
                more: 2
            }
        );
        assert_eq!(collapse(&v(&["a", "b", "c", "d", "e"]), 2).more, 4);
    }
}
