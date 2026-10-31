use chrono::FixedOffset;

use super::*;

/// 2026-10-30 21:14:00 UTC.
const EVENING: i64 = 1_793_394_840_000;

#[test]
fn a_start_time_is_shown_in_the_zone_given() {
    let utc = FixedOffset::east_opt(0).unwrap();
    assert_eq!(short_in(EVENING, &utc), "10/30 9:14 PM");
    let plus_three = FixedOffset::east_opt(3 * 3600).unwrap();
    assert_eq!(short_in(EVENING, &plus_three), "10/31 12:14 AM");
}

#[test]
fn a_time_that_is_out_of_range_is_empty() {
    assert_eq!(short_in(i64::MAX, &FixedOffset::east_opt(0).unwrap()), "");
}
