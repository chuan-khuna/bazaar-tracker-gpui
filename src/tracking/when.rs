//! Times as the player reads them, in local time.

use chrono::{DateTime, Local, TimeZone};

/// `10/30 9:14 PM`, in the local time zone.
pub fn short(ms: i64) -> String {
    short_in(ms, &Local)
}

pub fn short_in<Tz: TimeZone>(ms: i64, zone: &Tz) -> String
where
    Tz::Offset: std::fmt::Display,
{
    match DateTime::from_timestamp_millis(ms) {
        Some(utc) => utc.with_timezone(zone).format("%-m/%-d %-I:%M %p").to_string(),
        None => String::new(),
    }
}

#[cfg(test)]
#[path = "when_tests.rs"]
mod tests;
