//! The month grid of the cooking calendar: which days fill a month's weeks, and moving between
//! months. Weeks start on Monday.

use chrono::{Datelike, Duration, NaiveDate};

pub fn first_of(year: i32, month: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, 1).expect("a month from 1 to 12")
}

/// The month `delta` months from `(year, month)`.
pub fn shift(year: i32, month: u32, delta: i32) -> (i32, u32) {
    let index = year * 12 + month as i32 - 1 + delta;
    (index.div_euclid(12), index.rem_euclid(12) as u32 + 1)
}

/// The weeks that show the month: whole weeks, Monday to Sunday, so the first and last rows hold
/// days of the neighbouring months.
pub fn weeks(year: i32, month: u32) -> Vec<[NaiveDate; 7]> {
    let first = first_of(year, month);
    let start = first - Duration::days(i64::from(first.weekday().num_days_from_monday()));
    let (ny, nm) = shift(year, month, 1);
    let end = first_of(ny, nm);
    let mut out = Vec::new();
    let mut day = start;
    while day < end {
        out.push(std::array::from_fn(|i| day + Duration::days(i as i64)));
        day += Duration::days(7);
    }
    out
}

pub fn month_name(month: u32) -> &'static str {
    [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ][(month as usize + 11) % 12]
}

/// The first and last day of a month, for asking what was cooked in it.
pub fn bounds(year: i32, month: u32) -> (NaiveDate, NaiveDate) {
    let (ny, nm) = shift(year, month, 1);
    (first_of(year, month), first_of(ny, nm) - Duration::days(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn months_shift_across_year_ends_in_both_directions() {
        assert_eq!(shift(2026, 12, 1), (2027, 1));
        assert_eq!(shift(2026, 1, -1), (2025, 12));
        assert_eq!(shift(2026, 6, 0), (2026, 6));
        assert_eq!(shift(2026, 3, -15), (2024, 12));
        assert_eq!(shift(2026, 3, 25), (2028, 4));
    }

    #[test]
    fn the_grid_is_whole_monday_first_weeks_holding_every_day_of_the_month() {
        // November 2026 starts on a Sunday and ends on a Monday: six rows.
        let w = weeks(2026, 11);
        assert_eq!(w.len(), 6);
        assert_eq!(
            w[0][0],
            d(2026, 10, 26),
            "starts on the Monday before the 1st"
        );
        assert_eq!(w[0][6], d(2026, 11, 1));
        assert_eq!(w[5][0], d(2026, 11, 30));
        assert!(w.iter().all(|r| r[0].weekday().num_days_from_monday() == 0));
        let days: Vec<NaiveDate> = w.iter().flatten().copied().collect();
        for day in 1..=30 {
            assert!(days.contains(&d(2026, 11, day)), "{day}");
        }
        // Consecutive days, no gaps.
        assert!(days.windows(2).all(|p| p[1] - p[0] == Duration::days(1)));
    }

    #[test]
    fn a_month_that_fits_four_weeks_has_four_rows() {
        // February 2027 starts on a Monday and has 28 days.
        assert_eq!(weeks(2027, 2).len(), 4);
        assert_eq!(weeks(2027, 2)[0][0], d(2027, 2, 1));
    }

    #[test]
    fn bounds_and_names() {
        assert_eq!(
            bounds(2028, 2),
            (d(2028, 2, 1), d(2028, 2, 29)),
            "a leap year"
        );
        assert_eq!(bounds(2026, 12), (d(2026, 12, 1), d(2026, 12, 31)));
        assert_eq!(month_name(1), "January");
        assert_eq!(month_name(12), "December");
    }
}
