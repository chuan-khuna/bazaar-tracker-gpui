//! Times as the player reads them, in local time.

use chrono::{DateTime, Local, TimeZone};

/// `10/30 9:14 PM`, in the local time zone.
pub fn short(ms: i64) -> String {
    format_in(ms, &Local, "%-m/%-d %-I:%M %p")
}

/// `10/30/2026 9:14 PM`, in the local time zone.
pub fn long(ms: i64) -> String {
    format_in(ms, &Local, "%-m/%-d/%Y %-I:%M %p")
}

fn format_in<Tz: TimeZone>(ms: i64, zone: &Tz, pattern: &str) -> String
where
    Tz::Offset: std::fmt::Display,
{
    match DateTime::from_timestamp_millis(ms) {
        Some(utc) => utc.with_timezone(zone).format(pattern).to_string(),
        None => String::new(),
    }
}

#[cfg(test)]
#[path = "when_tests.rs"]
mod tests;
