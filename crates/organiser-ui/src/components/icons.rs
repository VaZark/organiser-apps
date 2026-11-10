//! App-owned icons, drawn the way iad draws its own (one colour, which the theme replaces), so
//! they never depend on what the user's icon theme has.

use crate::iad::IconSource;

/// Three lines with bullets: the same list, nothing nested.
pub fn flat_list() -> IconSource {
    IconSource::svg(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M6.5 4h7M6.5 8h7M6.5 12h7M2.5 4h.01M2.5 8h.01M2.5 12h.01"/></svg>"##,
    )
}

/// A circled check: tasks that are finished.
pub fn closed_tasks() -> IconSource {
    IconSource::svg(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="8" cy="8" r="6"/><path d="m5.3 8.2 1.9 1.9 3.5-3.7"/></svg>"##,
    )
}

/// Three columns of different heights: the kanban board.
pub fn kanban() -> IconSource {
    IconSource::svg(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="2.5" width="3.2" height="11" rx="1"/><rect x="6.4" y="2.5" width="3.2" height="7" rx="1"/><rect x="10.8" y="2.5" width="3.2" height="9" rx="1"/></svg>"##,
    )
}

/// One line per item: compact rows rather than cards.
pub fn compact_rows() -> IconSource {
    IconSource::svg(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2.5 4h11M2.5 8h11M2.5 12h11"/></svg>"##,
    )
}

/// Cards in columns: the masonry grid.
pub fn card_grid() -> IconSource {
    IconSource::svg(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="2.5" width="5" height="6" rx="1"/><rect x="9" y="2.5" width="5" height="3.5" rx="1"/><rect x="2" y="10.5" width="5" height="3" rx="1"/><rect x="9" y="8" width="5" height="5.5" rx="1"/></svg>"##,
    )
}
