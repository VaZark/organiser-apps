//! Tab and Shift+Tab move between the entries of a form or table.
//!
//! iced's entries do not move focus on Tab, and iad does it only inside a focus scope (every dialog
//! and docked sheet has one). A page that is not in one, such as a table of entries on the window
//! itself, is wrapped in [`tab_scope`]: Tab goes to the next entry in reading order and wraps from
//! the last to the first, and the page does not grab focus when it appears. The wrapper itself
//! lives in iad (`binds::tab_scope`); it is re-exported here so the apps have one place to look.

pub use crate::iad::binds::tab_scope;
