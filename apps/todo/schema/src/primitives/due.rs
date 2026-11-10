//! How near a due date is, and how to say so.

use chrono::NaiveDate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Bucket {
    Overdue,
    Today,
    /// Within the next week.
    Soon,
    Later,
}

pub fn bucket(due: NaiveDate, today: NaiveDate) -> Bucket {
    match (due - today).num_days() {
        d if d < 0 => Bucket::Overdue,
        0 => Bucket::Today,
        1..=7 => Bucket::Soon,
        _ => Bucket::Later,
    }
}

/// `Today`, `Tomorrow`, `in 3 days`, `2 days overdue`, or the date itself when far off.
pub fn label(due: NaiveDate, today: NaiveDate) -> String {
    let d = (due - today).num_days();
    match d {
        0 => "Today".into(),
        1 => "Tomorrow".into(),
        -1 => "Yesterday".into(),
        2..=7 => format!("in {d} days"),
        -30..=-2 => format!("{} days overdue", -d),
        _ => due.format("%-d %b %Y").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn buckets_and_labels() {
        let t = d("2026-11-10");
        assert_eq!(bucket(d("2026-11-09"), t), Bucket::Overdue);
        assert_eq!(bucket(d("2026-11-10"), t), Bucket::Today);
        assert_eq!(bucket(d("2026-11-17"), t), Bucket::Soon);
        assert_eq!(bucket(d("2026-11-18"), t), Bucket::Later);
        assert_eq!(label(d("2026-11-11"), t), "Tomorrow");
        assert_eq!(label(d("2026-11-13"), t), "in 3 days");
        assert_eq!(label(d("2026-11-07"), t), "3 days overdue");
        assert_eq!(label(d("2027-01-05"), t), "5 Jan 2027");
    }
}
