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

/// `9:20:11 PM`, in the local time zone.
pub fn clock(ms: i64) -> String {
    format_in(ms, &Local, CLOCK)
}

const CLOCK: &str = "%-I:%M:%S %p";

/// How far into a Run an Event is, as `m:ss` (`1:05`); past an hour, `h:mm:ss`. A time before the
/// start counts as the start.
pub fn into_run(ms: i64, started_ms: i64) -> String {
    let seconds = (ms - started_ms).max(0) / 1000;
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 { format!("{hours}:{minutes:02}:{seconds:02}") } else { format!("{minutes}:{seconds:02}") }
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
