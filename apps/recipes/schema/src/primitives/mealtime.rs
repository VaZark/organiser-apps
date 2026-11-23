//! When in the day a meal was: morning, brunch, lunch, dinner or evening.

use std::fmt;

use serde::{Deserialize, Serialize};

pub const NAMES: &[&str] = &["morning", "brunch", "lunch", "dinner", "evening"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MealTime {
    Morning,
    Brunch,
    Lunch,
    Dinner,
    Evening,
}

impl MealTime {
    /// In the order of the day.
    pub const ALL: [MealTime; 5] = [
        MealTime::Morning,
        MealTime::Brunch,
        MealTime::Lunch,
        MealTime::Dinner,
        MealTime::Evening,
    ];

    pub fn as_str(self) -> &'static str {
        NAMES[self as usize]
    }

    /// `morning`, `Lunch`, ` DINNER ` all parse.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().to_lowercase();
        Self::ALL.into_iter().find(|t| t.as_str() == s)
    }

    /// `Morning`, for showing.
    pub fn label(self) -> &'static str {
        match self {
            MealTime::Morning => "Morning",
            MealTime::Brunch => "Brunch",
            MealTime::Lunch => "Lunch",
            MealTime::Dinner => "Dinner",
            MealTime::Evening => "Evening",
        }
    }
}

impl fmt::Display for MealTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_in_the_order_of_the_day() {
        for t in MealTime::ALL {
            assert_eq!(MealTime::parse(t.as_str()), Some(t));
        }
        assert_eq!(MealTime::parse(" Lunch "), Some(MealTime::Lunch));
        assert_eq!(MealTime::parse("supper"), None);
        assert!(MealTime::Morning < MealTime::Evening);
        assert_eq!(NAMES.len(), MealTime::ALL.len());
    }
}
