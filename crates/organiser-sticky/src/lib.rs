//! # organiser-sticky
//!
//! Infra shared by the apps and the Sticky app. An item is a sticky when its `sticky` field is
//! true; the Sticky app owns no items, only a registry of linked apps' databases, and shows the
//! flagged items it finds in them.
//!
//! Layers (`tests/layers.rs` enforces them):
//!
//! * [`primitives`]: the registry model and the flag's field names, pure.
//! * [`components`]: the registry file, the flag on a [`organiser_engine::Db`], finding and
//!   starting the Sticky app.

pub mod components;
pub mod primitives;

pub use components::{autostart, discover, flag, store};
pub use primitives::local::{Local, LocalItem};
pub use primitives::registry::{KindSpec, Link, Links};

/// Application id of the Sticky app.
pub const STICKY_ID: &str = "io.github.vazark.Sticky";
/// Binary name of the Sticky app.
pub const STICKY_BIN: &str = "sticky-gui";
/// The bool field every stickable kind has in its schema.
pub const STICKY: &str = "sticky";
/// The optional `#rrggbb` field holding the sticky's colour.
pub const STICKY_COLOR: &str = "sticky_color";
