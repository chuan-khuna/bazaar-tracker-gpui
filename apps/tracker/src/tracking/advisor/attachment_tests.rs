use bazaar_tracker::memory::Snapshot;
use bazaar_tracker::run::{EventKind, PresentedOption};

use super::*;
use crate::tracking::test_events::{event, sold};

fn option(id: &str) -> PresentedOption {
    PresentedOption { instance_id: id.into(), kind: "Item".into(), size: "Small".into(), ..PresentedOption::default() }
}

#[test]
fn the_chip_names_the_events_number_time_kind_and_detail() {
    let mut started = event("start", 0, EventKind::RunStarted { play_mode: None, build_id: None, environment: None, hero: Some("Jules".into()), rating: None, rank: None, rank_division: None });
    started.day = Some(5);
    started.hour = Some(2);
    let offered = event("o", 1, EventKind::OptionsPresented { context: "ChoiceState".into(), options: vec![option("a"), option("b"), option("c")], snapshot: Snapshot::default() });
    let events = vec![started, offered, event("r", 2, EventKind::Rerolled), sold("s", 3, Snapshot::default())];
    let names = Names::default();
    // The Day and Hour carry over from the Event before.
    assert_eq!(chip_text(&events, "o", &names).as_deref(), Some("Event #2 · D5 H2 · Options: 3 options · Choice"));
    // An Event with no detail is its kind.
    assert_eq!(chip_text(&events, "r", &names).as_deref(), Some("Event #3 · D5 H2 · Rerolled"));
    assert_eq!(chip_text(&events, "s", &names).as_deref(), Some("Event #4 · D5 H2 · Sold: itm_x"));
    assert_eq!(chip_text(&events, "gone", &names), None);
}

#[test]
fn an_event_before_the_run_has_a_day_has_no_time() {
    let events = vec![event("r", 0, EventKind::Rerolled)];
    let attachment = Attachment::new("r1", &events, "r", &Names::default()).unwrap();
    assert_eq!(attachment, Attachment { run_id: "r1".into(), event_id: "r".into(), label: "Event #1 · Rerolled".into() });
}
