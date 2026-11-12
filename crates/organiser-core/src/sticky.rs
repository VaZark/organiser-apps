//! What an app tells Sticky Notes about itself so its text can be shown as stickies: the kinds of
//! item that can be stickies, and the field of each that holds their text. An app publishes one of
//! these (see `organiser-sticky`); Sticky Notes needs nothing else compiled in about the app.

use crate::AppMeta;

/// One kind of item that can be a sticky.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StickyKind {
    pub kind: &'static str,
    /// The Markdown field holding the item's text (also the one full-text search reads).
    pub body: &'static str,
}

/// An app's offer of stickies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StickySpec {
    pub meta: AppMeta,
    pub kinds: &'static [StickyKind],
}
