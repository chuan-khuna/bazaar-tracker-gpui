//! A batch of Events as it is sent: how many go in one, and how they are written and compressed.

use std::io::Write;

use flate2::Compression;
use flate2::write::GzEncoder;

use crate::history::WaitingEvent;
use crate::run::Run;

/// The most Events in one batch.
pub const MAX_BATCH_EVENTS: usize = 200;
/// About how many bytes of Event JSON go in one batch (before compression).
pub const MAX_BATCH_PAYLOAD_BYTES: usize = 2_000_000;

/// The Events that fit under `max_bytes` of JSON, in order. It stops at the first Event that would
/// go over; the first Event is always taken, even if it is over by itself.
pub fn limit_by_size(events: Vec<WaitingEvent>, max_bytes: usize) -> Vec<WaitingEvent> {
    let mut total = 0usize;
    let mut kept = Vec::with_capacity(events.len());
    for event in events {
        let size = event.json.len();
        if !kept.is_empty() && total + size > max_bytes {
            break;
        }
        total += size;
        kept.push(event);
    }
    kept
}

/// The JSON body of a batch: a schema version, who sent it, the Runs the Events belong to, and the
/// Events. The Event JSON is put in as it is.
pub fn build_body(client_version: &str, runs: &[Run], events: &[WaitingEvent]) -> String {
    let runs = serde_json::to_string(runs).unwrap_or_else(|_| "[]".to_owned());
    let version = serde_json::to_string(client_version).unwrap_or_else(|_| "\"\"".to_owned());
    let mut body = format!(r#"{{"schema_version":1,"client":"BazaarTracker","client_version":{version},"runs":{runs},"events":["#);
    for (index, event) in events.iter().enumerate() {
        if index > 0 {
            body.push(',');
        }
        body.push_str(&event.json);
    }
    body.push_str("]}");
    body
}

/// Compress text as a single standard gzip stream.
pub fn gzip(text: &str) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(text.as_bytes()).expect("writing to memory cannot fail");
    encoder.finish().expect("writing to memory cannot fail")
}

#[cfg(test)]
#[path = "batch_tests.rs"]
mod tests;
