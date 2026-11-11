//! Steps are items, and the engine wants every item to have a title. A step's is made from its text.

/// The first line of the step that says something, cut to a short title; `Step` when it is empty.
pub fn title_of(text: &str) -> String {
    let line = text
        .lines()
        .map(|l| {
            l.trim()
                .trim_start_matches(['#', '-', '*', '>', ' '])
                .trim()
        })
        .find(|l| !l.is_empty())
        .unwrap_or("Step");
    let mut t: String = line.chars().take(60).collect();
    if line.chars().count() > 60 {
        t.push('…');
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_is_the_first_real_line_without_markdown_markers() {
        assert_eq!(title_of("Chop the onions\nthen fry"), "Chop the onions");
        assert_eq!(title_of("\n\n  ## Make the dough"), "Make the dough");
        assert_eq!(title_of("- [ ] stir"), "[ ] stir");
    }

    #[test]
    fn an_empty_step_is_just_step_and_a_long_line_is_cut() {
        assert_eq!(title_of(""), "Step");
        assert_eq!(title_of("  \n \n"), "Step");
        let long = "x".repeat(100);
        let t = title_of(&long);
        assert_eq!(t.chars().count(), 61);
        assert!(t.ends_with('…'));
    }
}
