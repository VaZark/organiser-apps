//! # sticky-engine
//!
//! Everything Sticky Notes *does*, with no UI: which apps are linked (the registry the apps and
//! third parties write, see `organiser-sticky`), which of their items are stickies, editing one,
//! and dropping one. The GUI and the command line are frontends over [`Engine`].
//!
//! Sticky owns no items. Each sticky is an item of a linked app's database, and the app alone says
//! that it is one (its `sticky` flag, which Sticky only reads). Edits to the title and text are
//! written straight back to the item. How a sticky shows (on the desktop or only in Sticky's own
//! window, and its colour) is Sticky's own, kept apart from the app's data.
//!
//! Layers (`tests/layers.rs` enforces them): [`primitives`] are plain data; [`components`] touch
//! files and databases.

pub mod components;
pub mod primitives;

pub use components::engine::Engine;
pub use components::watch::Watcher;
pub use primitives::item::{Changes, Key, StickyItem};
