# 1. One date and time style across the app

Date: 2026-11-01

## Status

Accepted

## Context

The app showed times in four styles: `11/1 8:25 PM` in the runs list, `11/1/2026 8:25 PM` in the
Run header and the shop odds' "Saved", `8:25:07 PM` in an Event's detail, and `20:25:07` in the log.
That mixed 12- and 24-hour clocks, and `11/1` reads as 11 January to most players outside the US.
The offset of an Event into its Run (`1:05`) looked like a time of day.

## Decision

Every time the app shows comes from `apps/tracker/src/tracking/when.rs`, in one style: a 24-hour clock, the day
before the month's short English name, and no dependence on the system locale.

| Function    | Shape                          | Example                    | Used for                         |
|-------------|--------------------------------|----------------------------|----------------------------------|
| `time_s`    | `%H:%M:%S`                     | `20:25:07`                 | log rows, Last event, Event detail |
| `date_time` | `%-d %b %H:%M`, year if not this year | `1 Nov 20:25`, `30 Oct 2025 20:25` | runs list, Run header |
| `stamp`     | `%-d %b %Y %H:%M`              | `1 Nov 2026 20:25`         | when something was saved         |
| `offset`    | `+m:ss`, `+h:mm:ss` past an hour | `+1:05`                  | an Event's time into its Run     |

Stored times stay as UTC milliseconds, and the log file keeps tracing's RFC 3339 UTC times.

## Consequences

- A date reads one way for every player, and times sort as they read.
- Tests give the same strings on every machine.
- Players who prefer a 12-hour clock or their own locale's order do not get it; if they ask, a
  setting in `when.rs` can choose between styles without touching the callers.
