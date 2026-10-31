use chrono::FixedOffset;

use super::*;

/// 2026-10-30 21:14:00 UTC.
const EVENING: i64 = 1_793_394_840_000;

#[test]
fn a_start_time_is_shown_in_the_zone_given() {
    let utc = FixedOffset::east_opt(0).unwrap();
    assert_eq!(format_in(EVENING, &utc, "%-m/%-d %-I:%M %p"), "10/30 9:14 PM");
    assert_eq!(format_in(EVENING, &utc, "%-m/%-d/%Y %-I:%M %p"), "10/30/2026 9:14 PM");
    let plus_three = FixedOffset::east_opt(3 * 3600).unwrap();
    assert_eq!(format_in(EVENING, &plus_three, "%-m/%-d %-I:%M %p"), "10/31 12:14 AM");
}

#[test]
fn a_time_that_is_out_of_range_is_empty() {
    assert_eq!(format_in(i64::MAX, &FixedOffset::east_opt(0).unwrap(), "%H"), "");
}

#[test]
fn the_local_formats_have_the_shape_the_sidebar_and_header_expect() {
    assert!(short(EVENING).ends_with("AM") || short(EVENING).ends_with("PM"));
    assert!(long(EVENING).contains("/2026 "));
}

#[test]
fn the_clock_has_seconds() {
    let utc = FixedOffset::east_opt(0).unwrap();
    assert_eq!(format_in(EVENING + 11_000, &utc, CLOCK), "9:14:11 PM");
    assert!(clock(EVENING).contains(':'));
}

#[test]
fn time_into_a_run_is_minutes_and_seconds() {
    assert_eq!(into_run(1_000, 1_000), "0:00");
    assert_eq!(into_run(13_900, 1_000), "0:12");
    assert_eq!(into_run(66_000, 1_000), "1:05");
    assert_eq!(into_run(601_000, 1_000), "10:00");
}

#[test]
fn time_into_a_run_past_an_hour_has_hours() {
    assert_eq!(into_run(3_726_000, 0), "1:02:06");
}

#[test]
fn a_time_before_the_start_is_the_start() {
    assert_eq!(into_run(500, 1_000), "0:00");
}

#[test]
fn the_log_time_is_twenty_four_hour_with_seconds() {
    let utc = FixedOffset::east_opt(0).unwrap();
    assert_eq!(format_in(EVENING + 7_000, &utc, HMS), "21:14:07");
    assert_eq!(hms(EVENING).len(), 8);
}
