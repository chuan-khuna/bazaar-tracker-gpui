//! Times as the player reads them, in local time. Every time the app shows comes from here, in one
//! style (see docs/adr/0001-one-date-and-time-style.md): 24-hour clock, the day before the month's
//! short name, and the year only when it is not this year.

use chrono::{DateTime, Datelike, Local, TimeZone};

/// `20:25`.
const TIME: &str = "%H:%M";
/// `20:25:07`.
const TIME_S: &str = "%H:%M:%S";
/// `1 Nov`.
const DATE: &str = "%-d %b";
/// `1 Nov 2025`.
const DATE_YEAR: &str = "%-d %b %Y";

/// `20:25:07`: for the log and an Event's own time, where seconds matter.
pub fn time_s(ms: i64) -> String {
    format_in(ms, &Local, TIME_S)
}

/// `1 Nov 20:25`, or `1 Nov 2025 20:25` in another year: when a Run started.
pub fn date_time(ms: i64) -> String {
    date_time_in(ms, now_ms(), &Local)
}

/// `1 Nov 2026 20:25`, always with the year: when something was saved.
pub fn stamp(ms: i64) -> String {
    format_in(ms, &Local, "%-d %b %Y %H:%M")
}

/// How far into a Run an Event is, as `+m:ss` (`+1:05`); past an hour, `+h:mm:ss`. The `+` tells
/// it from a time of day. A time before the start counts as the start.
pub fn offset(ms: i64, started_ms: i64) -> String {
    let seconds = (ms - started_ms).max(0) / 1000;
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 { format!("+{hours}:{minutes:02}:{seconds:02}") } else { format!("+{minutes}:{seconds:02}") }
}

fn date_time_in<Tz: TimeZone>(ms: i64, now_ms: i64, zone: &Tz) -> String
where
    Tz::Offset: std::fmt::Display,
{
    let year = |ms: i64| DateTime::from_timestamp_millis(ms).map(|utc| utc.with_timezone(zone).year());
    let date = if year(ms) == year(now_ms) { DATE } else { DATE_YEAR };
    format_in(ms, zone, &format!("{date} {TIME}"))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis() as i64)
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
