//! The search a header opens: whether it is showing, and what was typed.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Search {
    open: bool,
    text: String,
}

impl Search {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// The text to search for: `None` when nothing but whitespace was typed.
    pub fn query(&self) -> Option<&str> {
        let t = self.text.trim();
        (!t.is_empty()).then_some(t)
    }

    /// Open it, or close it and forget what was typed.
    pub fn toggle(&mut self) {
        if self.open {
            self.close();
        } else {
            self.open = true;
        }
    }

    pub fn close(&mut self) {
        self.open = false;
        self.text.clear();
    }

    pub fn input(&mut self, text: String) {
        self.text = text;
    }
}

/// Room one header button takes, with the gap beside it.
const SLOT: f32 = 42.0;
/// The window's close button is a little narrower than a slot.
const CLOSE_TRIM: f32 = 20.0;

/// The least width the search entry should keep. It is 420 wide when there is room and shrinks to
/// fit when there is not, but no further than this.
const MIN_ENTRY: f32 = 300.0;

/// How wide a spacer on the start side of a header must be to put the search entry in the middle.
/// The header centres its title between the start and end groups, so a heavier end group pulls the
/// entry off centre by half the difference. `end_buttons` are the buttons packed at the end (not
/// counting the window's close button, which is always there at the end); `start_buttons` are
/// those already packed at the start; `room` is the header's width.
///
/// When the spacer would squeeze the entry below [`MIN_ENTRY`] the entry matters more than being
/// centred, so there is no spacer.
pub fn balance_width(end_buttons: usize, start_buttons: usize, room: f32) -> f32 {
    let end = (end_buttons + 1) as f32 * SLOT - CLOSE_TRIM;
    let spacer = (end - start_buttons as f32 * SLOT).max(0.0);
    if room - end - spacer < MIN_ENTRY {
        0.0
    } else {
        spacer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_forgets_the_text_and_blank_is_no_query() {
        let mut s = Search::default();
        s.toggle();
        s.input("  milk ".into());
        assert!(s.is_open());
        assert_eq!(s.query(), Some("milk"));
        s.input("   ".into());
        assert_eq!(s.query(), None);
        s.input("x".into());
        s.toggle();
        assert!(!s.is_open());
        assert_eq!(s.text(), "");
    }

    #[test]
    fn the_spacer_balances_the_end_group_and_never_goes_negative() {
        // Five buttons and the close button at the end, nothing at the start.
        assert_eq!(balance_width(5, 0, 1200.0), 232.0);
        // A button already at the start covers one slot of it.
        assert_eq!(balance_width(5, 1, 1200.0), 190.0);
        assert_eq!(balance_width(0, 9, 1200.0), 0.0);
        assert!(balance_width(3, 0, 1200.0) < balance_width(5, 0, 1200.0));
    }

    #[test]
    fn a_narrow_header_keeps_the_entry_wide_instead_of_centring_it() {
        assert_eq!(balance_width(5, 0, 430.0), 0.0);
        // 800 wide still has room for a centred entry (a little narrower than usual).
        assert_eq!(balance_width(5, 0, 800.0), 232.0);
        // Just enough room for the entry beside the end group and the spacer, and one pixel less.
        assert_eq!(balance_width(5, 0, 232.0 + 232.0 + MIN_ENTRY), 232.0);
        assert_eq!(balance_width(5, 0, 232.0 + 232.0 + MIN_ENTRY - 1.0), 0.0);
    }
}
