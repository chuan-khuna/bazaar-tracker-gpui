use bazaar_tracker::run::Outcome;

use super::*;
use crate::tracking::test_events::*;

fn run() -> Vec<Event> {
    vec![
        event("started", 0, EventKind::RunStarted { play_mode: None, build_id: None, environment: None, hero: None, rating: None, rank: None, rank_division: None }),
        options("options", 10, snapshot(10, &[])),
        sold("sold", 20, snapshot(20, &[])),
        combat_start("fight", 30, snapshot(30, &[]), snapshot(99, &[])),
        combat_end("fight-over", 40, Outcome::Win),
        event("reroll", 50, EventKind::Rerolled),
    ]
}

#[test]
fn with_nothing_selected_it_is_the_latest_snapshot() {
    assert_eq!(gold_of(at(&run(), None)), Some(30));
}

#[test]
fn a_combat_start_contributes_the_players_side() {
    let events = run();
    assert_eq!(gold_of(at(&events, Some("fight"))), Some(30));
}

#[test]
fn a_selected_event_with_a_snapshot_shows_its_own() {
    let events = run();
    assert_eq!(gold_of(at(&events, Some("options"))), Some(10));
    assert_eq!(gold_of(at(&events, Some("sold"))), Some(20));
}

#[test]
fn a_selected_event_without_one_shows_the_nearest_before_it() {
    let events = run();
    assert_eq!(gold_of(at(&events, Some("fight-over"))), Some(30));
    assert_eq!(gold_of(at(&events, Some("reroll"))), Some(30));
}

#[test]
fn an_event_before_any_snapshot_shows_none() {
    let events = run();
    assert!(at(&events, Some("started")).is_none());
}

#[test]
fn an_unknown_selection_counts_as_none() {
    assert_eq!(gold_of(at(&run(), Some("gone"))), Some(30));
}

#[test]
fn a_run_with_no_events_has_no_snapshot() {
    assert!(at(&[], None).is_none());
}
