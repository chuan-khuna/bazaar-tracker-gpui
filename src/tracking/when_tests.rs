use chrono::FixedOffset;

use super::*;

/// 2026-10-30 21:14:00 UTC.
const EVENING: i64 = 1_793_394_840_000;

/// A year before `EVENING`.
const LAST_YEAR: i64 = EVENING - 365 * 24 * 3600 * 1000;

fn utc() -> FixedOffset {
    FixedOffset::east_opt(0).unwrap()
}

#[test]
fn a_start_time_this_year_leaves_the_year_out() {
    assert_eq!(date_time_in(EVENING, EVENING, &utc()), "30 Oct 21:14");
}

#[test]
fn a_start_time_in_another_year_has_the_year() {
    assert_eq!(date_time_in(LAST_YEAR, EVENING, &utc()), "30 Oct 2025 21:14");
}

#[test]
fn a_time_is_shown_in_the_zone_given() {
    let plus_three = FixedOffset::east_opt(3 * 3600).unwrap();
    assert_eq!(date_time_in(EVENING, EVENING, &plus_three), "31 Oct 00:14");
}

#[test]
fn the_year_is_the_one_in_the_zone_given() {
    // 31 Dec 2026 23:30 UTC is already 2027 three hours east; "now" there is in 2027 too.
    let new_years_eve = 1_798_759_800_000;
    let plus_three = FixedOffset::east_opt(3 * 3600).unwrap();
    assert_eq!(date_time_in(new_years_eve, new_years_eve, &plus_three), "1 Jan 02:30");
    assert_eq!(date_time_in(new_years_eve, EVENING, &plus_three), "1 Jan 2027 02:30");
}

#[test]
fn a_time_that_is_out_of_range_is_empty() {
    assert_eq!(format_in(i64::MAX, &utc(), TIME), "");
}

#[test]
fn the_formats_have_the_agreed_shapes() {
    assert_eq!(format_in(EVENING + 7_000, &utc(), TIME_S), "21:14:07");
    assert_eq!(format_in(EVENING, &utc(), "%-d %b %Y %H:%M"), "30 Oct 2026 21:14");
    assert_eq!(time_s(EVENING).len(), 8);
    assert!(stamp(EVENING).contains(" 2026 "));
}

#[test]
fn an_offset_into_a_run_is_minutes_and_seconds() {
    assert_eq!(offset(1_000, 1_000), "+0:00");
    assert_eq!(offset(13_900, 1_000), "+0:12");
    assert_eq!(offset(66_000, 1_000), "+1:05");
    assert_eq!(offset(601_000, 1_000), "+10:00");
}

#[test]
fn an_offset_past_an_hour_has_hours() {
    assert_eq!(offset(3_726_000, 0), "+1:02:06");
}

#[test]
fn a_time_before_the_start_is_the_start() {
    assert_eq!(offset(500, 1_000), "+0:00");
}
