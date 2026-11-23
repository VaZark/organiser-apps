//! How much a task matters: Jira's five levels.

use std::fmt;

use serde::{Deserialize, Serialize};

pub const NAMES: &[&str] = &["lowest", "low", "medium", "high", "highest"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Lowest,
    Low,
    #[default]
    Medium,
    High,
    Highest,
}

impl Priority {
    pub const ALL: [Priority; 5] = [
        Priority::Highest,
        Priority::High,
        Priority::Medium,
        Priority::Low,
        Priority::Lowest,
    ];

    pub fn as_str(self) -> &'static str {
        NAMES[self as usize]
    }

    pub fn label(self) -> &'static str {
        match self {
            Priority::Lowest => "Lowest",
            Priority::Low => "Low",
            Priority::Medium => "Medium",
            Priority::High => "High",
            Priority::Highest => "Highest",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == s)
    }

    /// Worth drawing attention to: high or highest.
    pub fn is_urgent(self) -> bool {
        self >= Priority::High
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_names_and_urgency() {
        assert!(Priority::Highest > Priority::Low);
        for p in Priority::ALL {
            assert_eq!(Priority::parse(p.as_str()), Some(p));
        }
        assert!(Priority::High.is_urgent() && !Priority::Medium.is_urgent());
        assert_eq!(Priority::default(), Priority::Medium);
    }
}
