use std::io::Read;

use super::*;
use crate::run::{Event, EventKind};

fn waiting(id: &str, padding: usize) -> WaitingEvent {
    let event = Event { id: id.to_owned(), run_id: "r1".into(), timestamp_ms: 1, day: None, hour: None, kind: EventKind::Rerolled };
    let mut json = serde_json::to_string(&event).unwrap();
    // Pad the JSON to a chosen length with spaces before the closing brace.
    while json.len() < padding {
        json.insert(json.len() - 1, ' ');
    }
    WaitingEvent { event, json }
}

#[test]
fn a_batch_keeps_events_while_they_fit_and_stops_at_the_first_that_does_not() {
    let events = vec![waiting("a", 400), waiting("b", 400), waiting("c", 400), waiting("d", 100)];
    let kept = limit_by_size(events, 1000);
    // 400 + 400 = 800 fits; the third would make 1200, so it stops there even though "d" is small.
    assert_eq!(kept.iter().map(|e| e.event.id.as_str()).collect::<Vec<_>>(), ["a", "b"]);
}

#[test]
fn the_cap_counts_bytes_exactly() {
    let events = vec![waiting("a", 500), waiting("b", 500), waiting("c", 500)];
    assert_eq!(limit_by_size(events.clone(), 1000).len(), 2, "exactly at the cap is allowed");
    assert_eq!(limit_by_size(events, 999).len(), 1);
}

#[test]
fn the_first_event_is_always_taken_even_if_it_is_over_the_cap() {
    let kept = limit_by_size(vec![waiting("big", 5000), waiting("next", 100)], 1000);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].event.id, "big");
    assert!(limit_by_size(Vec::new(), 1000).is_empty());
}

#[test]
fn the_limits_are_those_of_the_spec() {
    assert_eq!(MAX_BATCH_EVENTS, 200);
    assert_eq!(MAX_BATCH_PAYLOAD_BYTES, 2_000_000);
}

#[test]
fn the_body_is_json_with_the_runs_and_the_events_as_they_are() {
    let run = Run { id: "r1".into(), started_ms: 5, ended: true, ..Run::default() };
    let events = vec![waiting("a", 0), waiting("b", 0)];
    let body = build_body("0.1.0", std::slice::from_ref(&run), &events);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["schema_version"], 1);
    assert_eq!(parsed["client"], "BazaarTracker");
    assert_eq!(parsed["client_version"], "0.1.0");
    assert_eq!(parsed["runs"][0]["id"], "r1");
    assert_eq!(parsed["events"].as_array().unwrap().len(), 2);
    assert_eq!(parsed["events"][1]["id"], "b");
    // The event text is in the body untouched.
    assert!(body.contains(&events[0].json));
}

#[test]
fn a_body_with_no_events_or_runs_is_still_valid_json() {
    let body = build_body("0.1.0", &[], &[]);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(parsed["events"].as_array().unwrap().is_empty() && parsed["runs"].as_array().unwrap().is_empty());
}

#[test]
fn the_version_is_escaped() {
    let body = build_body("1.0 \"beta\"", &[], &[]);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(parsed["client_version"], "1.0 \"beta\"");
}

#[test]
fn gzip_round_trips_as_one_standard_stream() {
    let text = "{\"hello\":\"wörld\"}".repeat(100);
    let compressed = gzip(&text);
    assert_eq!(&compressed[..2], &[0x1f, 0x8b], "gzip magic");
    assert!(compressed.len() < text.len());
    let mut back = String::new();
    flate2::read::GzDecoder::new(&compressed[..]).read_to_string(&mut back).unwrap();
    assert_eq!(back, text);
}
