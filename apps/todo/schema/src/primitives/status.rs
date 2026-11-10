//! Where a task is in its life, as in Jira: not started, in progress, done, or won't do.

use std::fmt;

use serde::{Deserialize, Serialize};

pub const NAMES: &[&str] = &["open", "in_progress", "done", "dismissed"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[default]
    Open,
    InProgress,
    Done,
    Dismissed,
}

impl Status {
    pub const ALL: [Status; 4] = [
        Status::Open,
        Status::InProgress,
        Status::Done,
        Status::Dismissed,
    ];

    /// The stored name.
    pub fn as_str(self) -> &'static str {
        NAMES[self as usize]
    }

    /// The name a person reads.
    pub fn label(self) -> &'static str {
        match self {
            Status::Open => "To do",
            Status::InProgress => "In progress",
            Status::Done => "Done",
            Status::Dismissed => "Won't do",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == s)
    }

    /// Not finished: to do or in progress.
    pub fn is_open(self) -> bool {
        matches!(self, Self::Open | Self::InProgress)
    }

    pub fn is_closed(self) -> bool {
        !self.is_open()
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_and_open_means_unfinished() {
        for s in Status::ALL {
            assert_eq!(Status::parse(s.as_str()), Some(s));
        }
        assert!(Status::InProgress.is_open() && Status::Open.is_open());
        assert!(Status::Done.is_closed() && Status::Dismissed.is_closed());
        assert_eq!(Status::parse("nope"), None);
    }
}
