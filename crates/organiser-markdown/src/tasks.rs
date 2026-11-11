//! Ticking a task in the source: the checkbox of the `n`th task item (reading order, as the
//! rendered document numbers them) is rewritten in the Markdown text, and nothing else changes.

/// The task marker at the start of a list line: where its `[` is, and whether it is checked.
fn task_at(line: &str) -> Option<(usize, bool)> {
    // Quotes may wrap a list: skip any number of `>`.
    let mut i = 0;
    let b = line.as_bytes();
    loop {
        while b.get(i) == Some(&b' ') {
            i += 1;
        }
        if b.get(i) == Some(&b'>') {
            i += 1;
        } else {
            break;
        }
    }
    match b.get(i)? {
        b'-' | b'*' | b'+' => i += 1,
        b'0'..=b'9' => {
            while b.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
            if !matches!(b.get(i), Some(b'.' | b')')) {
                return None;
            }
            i += 1;
        }
        _ => return None,
    }
    if b.get(i) != Some(&b' ') {
        return None;
    }
    while b.get(i) == Some(&b' ') {
        i += 1;
    }
    let mark = b.get(i + 1)?;
    let closes = b.get(i + 2) == Some(&b']') && matches!(b.get(i + 3), Some(b' ') | None);
    (b[i] == b'[' && closes && matches!(mark, b' ' | b'x' | b'X')).then_some((i, *mark != b' '))
}

/// How many task items the source has, not counting lines inside fenced code.
pub fn count(md: &str) -> usize {
    tasks(md).count()
}

fn tasks(md: &str) -> impl Iterator<Item = (usize, usize, bool)> + '_ {
    let mut fence: Option<(char, usize)> = None;
    let mut offset = 0;
    md.split_inclusive('\n').filter_map(move |line| {
        let start = offset;
        offset += line.len();
        let t = line.trim_start();
        let run = |c: char| t.chars().take_while(|x| *x == c).count();
        match fence {
            Some((c, n)) => {
                if run(c) >= n && t.trim_start_matches(c).trim().is_empty() {
                    fence = None;
                }
                None
            }
            None => {
                for c in ['`', '~'] {
                    if run(c) >= 3 {
                        fence = Some((c, run(c)));
                        return None;
                    }
                }
                task_at(line).map(|(col, on)| (start, col, on))
            }
        }
    })
}

/// `md` with task `n` set to `checked`, or `None` if there is no such task.
pub fn set(md: &str, n: usize, checked: bool) -> Option<String> {
    let (start, col, _) = tasks(md).nth(n)?;
    let at = start + col + 1;
    let mut out = String::with_capacity(md.len());
    out.push_str(&md[..at]);
    out.push(if checked { 'x' } else { ' ' });
    out.push_str(&md[at + 1..]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "- [ ] one\n- [x] two\n  - [ ] nested\n\n```\n- [ ] not a task\n```\n\n> 1. [X] quoted\n- [ ]no space\n";

    #[test]
    fn counts_real_tasks_only() {
        assert_eq!(count(SRC), 4);
    }

    #[test]
    fn sets_and_clears_the_nth_task_and_touches_nothing_else() {
        let a = set(SRC, 0, true).unwrap();
        assert_eq!(a, SRC.replacen("- [ ] one", "- [x] one", 1));
        let b = set(SRC, 1, false).unwrap();
        assert_eq!(b, SRC.replacen("- [x] two", "- [ ] two", 1));
        assert!(set(SRC, 2, true).unwrap().contains("  - [x] nested"));
        assert!(set(SRC, 3, false).unwrap().contains("> 1. [ ] quoted"));
    }

    #[test]
    fn a_missing_task_is_none() {
        assert_eq!(set(SRC, 4, true), None);
        assert_eq!(set("plain text", 0, true), None);
    }
}
