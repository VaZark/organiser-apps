//! Where an entry is, in lists that track it.

use std::fmt;

use serde::{Deserialize, Serialize};

pub const NAMES: &[&str] = &["planned", "active", "done"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[default]
    Planned,
    Active,
    Done,
}

impl Status {
    pub const ALL: [Status; 3] = [Status::Planned, Status::Active, Status::Done];

    pub fn as_str(self) -> &'static str {
        NAMES[self as usize]
    }

    pub fn label(self) -> &'static str {
        match self {
            Status::Planned => "Planned",
            Status::Active => "In progress",
            Status::Done => "Done",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == s)
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
    fn names_round_trip() {
        for s in Status::ALL {
            assert_eq!(Status::parse(s.as_str()), Some(s));
        }
        assert_eq!(Status::parse("watching"), None);
    }
}
