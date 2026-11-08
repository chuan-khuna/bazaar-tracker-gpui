use std::time::Duration;

use bazaar_tracker::run::{Event, EventKind};

use super::*;

fn connected() -> Notice {
    Notice::Connection(ConnectionChange::Connected { pid: 1, unity_version: "6".into(), details: Default::default() })
}

fn disconnected() -> Notice {
    Notice::Connection(ConnectionChange::Disconnected { reason: "gone".into() })
}

fn save_failed(what: &str, retry_in: Duration) -> Notice {
    Notice::SaveFailed { what: what.into(), error: "disk full".into(), retry_in }
}

fn saved() -> Notice {
    Notice::EventSaved(Event { id: "e1".into(), run_id: "r1".into(), timestamp_ms: 0, day: None, hour: None, kind: EventKind::Rerolled })
}

#[test]
fn a_service_that_started_waits_for_the_game_and_one_that_did_not_is_not_tracking() {
    assert_eq!(Status::new(true).connection, Connection::Waiting);
    assert_eq!(Status::new(false).connection, Connection::NotTracking);
}

#[test]
fn attaching_and_detaching_switch_between_tracking_and_waiting() {
    let mut status = Status::new(true);
    assert_eq!(status.apply(&connected()), None);
    assert_eq!(status.connection, Connection::Tracking);
    assert_eq!(status.apply(&disconnected()), None);
    assert_eq!(status.connection, Connection::Waiting);
}

#[test]
fn the_labels_are_the_words_of_the_status_bar() {
    assert_eq!(Connection::Tracking.label(), "Tracking");
    assert_eq!(Connection::Waiting.label(), "Waiting for the game");
    assert_eq!(Connection::NotTracking.label(), "Not tracking");
}

#[test]
fn a_failed_save_is_an_error_naming_what_and_why() {
    let mut status = Status::new(true);
    let error = status.apply(&save_failed("event e1 (Sold)", Duration::from_secs(1))).unwrap();
    assert_eq!(error, "Could not save event e1 (Sold): disk full");
}

#[test]
fn a_log_error_is_an_error() {
    let mut status = Status::new(true);
    assert_eq!(status.apply(&Notice::LogError("access denied".into())).unwrap(), "Could not read Player.log: access denied");
}

#[test]
fn the_same_error_is_not_reported_twice_even_with_a_longer_retry() {
    let mut status = Status::new(true);
    assert!(status.apply(&save_failed("event e1 (Sold)", Duration::from_secs(1))).is_some());
    assert_eq!(status.apply(&save_failed("event e1 (Sold)", Duration::from_secs(2))), None);
    assert_eq!(status.apply(&save_failed("event e1 (Sold)", Duration::from_secs(4))), None);
}

#[test]
fn a_different_error_is_reported() {
    let mut status = Status::new(true);
    assert!(status.apply(&save_failed("event e1 (Sold)", Duration::from_secs(1))).is_some());
    assert!(status.apply(&save_failed("event e2 (Sold)", Duration::from_secs(1))).is_some());
    assert!(status.apply(&Notice::LogError("x".into())).is_some());
}

#[test]
fn a_saved_event_ends_the_problem_so_the_same_error_can_come_back() {
    let mut status = Status::new(true);
    assert!(status.apply(&Notice::LogError("x".into())).is_some());
    assert_eq!(status.apply(&Notice::LogError("x".into())), None);
    assert_eq!(status.apply(&saved()), None);
    assert!(status.apply(&Notice::LogError("x".into())).is_some());
}
