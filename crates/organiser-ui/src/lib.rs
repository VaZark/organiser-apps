//! # organiser-ui
//!
//! What every organiser GUI shares, built on [`iced_adaptive_design`] (`iad`). Layers, each
//! importing only the ones above it in this list (`tests/layers.rs` enforces it):
//!
//! * [`primitives`]: headless UI state and rules (expansion, theme mode). No iced.
//! * [`components`]: small themed pieces with no information of their own (tag chip, empty state).
//! * [`widgets`]: what the user sees: the item tree, the tag sidebar entries.
//! * [`shell`]: window settings, theme choice and the desktop entry, for `main`.

pub mod components;
pub mod primitives;
pub mod shell;
pub mod widgets;

pub use iced_adaptive_design as iad;

/// The element type of every organiser view.
pub type Element<'a, M> = iced::Element<'a, M, iad::Theme>;
